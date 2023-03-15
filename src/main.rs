use std::io::Write as _;

#[derive(Clone)]
#[repr(transparent)]
struct Value(serde_json::Value);

impl Value {
    #[track_caller]
    fn i(&self) -> i64 {
        self.0.as_i64().unwrap()
    }
    #[track_caller]
    fn s(&self) -> &str {
        self.0.as_str().unwrap()
    }
    #[track_caller]
    fn f(&self) -> f64 {
        self.0.as_f64().unwrap()
    }
    #[track_caller]
    fn a(&self) -> &[Value] {
        let a = self.0.as_array().unwrap().as_slice();
        unsafe { std::mem::transmute(a) }
    }
}

impl<K: serde_json::value::Index> std::ops::Index<K> for Value {
    type Output = Value;

    fn index(&self, index: K) -> &Self::Output {
        let v = &self.0[index];
        unsafe { std::mem::transmute(v) }
    }
}

fn usage(short: bool) {
    println!(
        "usage: graf [-h|--help] <-u USER:PASS|-t TOKEN> URL [--from FROM] [--to TO] [--interval SECS] [-f]"
    );
    if short {
        return;
    }
    print!(
        r#"
  select and print grafana dashboard panel to terminal

  -u USER:PASS basic user password authentication
  -t TOKEN     api token
  URL          grafana base url
  --from FROM, --to TO
               time specifiers for grafana (defaults to now-1m, now)
  INTERVAL    interval in seconds between frames (defaults to <terminal rows> / TO-FROM)
  -f           follow, update data every INTERVAL seconds
"#
    );
    println!(
        "{} {} by {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_AUTHORS")
    );
}

pub fn timestamp<'b>(time_ms: i64, buf: &'b mut [u8; 9]) -> &'b str {
    let time = libc::time_t::from(time_ms / 1000);

    // SAFETY: C ffi stuff
    unsafe {
        let mut tm_buf = std::mem::zeroed::<libc::tm>();
        let tm_ptr = (&mut tm_buf) as *mut _;
        assert_eq!(libc::gmtime_r(&time, tm_ptr), tm_ptr, "gmtime(_, p) != p");

        let (buf, max) = (buf.as_mut_ptr() as _, buf.len());
        let date_fmt = "%H:%M:%S\0".as_ptr() as _;
        let n = libc::strftime(buf, max, date_fmt, &tm_buf);
        assert_eq!(n, 8, "stftime(_, _, \"%H:%M:%S\", _) != \"13:04:05\".len()");
    }

    // SAFETY: asserted strftime wrote 8 bytes of valid ascii
    unsafe { std::str::from_utf8_unchecked(&buf[..8]) }
}

pub fn parse_instant(time_s: &str, now: i64) -> Option<i64> {
    let time_s = time_s.trim();
    if time_s.chars().all(char::is_numeric) {
        let time: i64 = time_s.parse().ok()?;
        Some(time)
    } else if time_s.len() == "20160201T130405".len()
        && &time_s["20160201".len().."20160201T".len()] == "T"
    {
        let (year, time_s) = time_s.split_at("2016".len());
        let (month, time_s) = time_s.split_at("02".len());
        let (day, time_s) = time_s.split_at("01".len());
        let (_t, time_s) = time_s.split_at("T".len());
        let (hour, time_s) = time_s.split_at("13".len());
        let (minute, time_s) = time_s.split_at("04".len());
        let (second, _time_s) = time_s.split_at("05".len());
        let mut tm = libc::tm {
            tm_sec: second.parse().ok()?,
            tm_min: minute.parse().ok()?,
            tm_hour: hour.parse().ok()?,
            tm_mday: day.parse().ok()?,
            tm_mon: month.parse().ok()?,
            tm_year: year.parse().ok()?,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: -1, // a negative value indicates mktime should use tz db to determine daylight savings
            tm_gmtoff: 0,
            tm_zone: "UTC".as_ptr() as _,
        };
        // SAFETY: ffi
        let time = unsafe { libc::mktime(&mut tm as _) };
        Some(time)
    } else if time_s.len() >= "now".len() && &time_s[..3] == "now" {
        let (_now, time_s) = time_s.split_at("now".len());
        let time_s = time_s.trim_start();
        match time_s.chars().next() {
            None => Some(now),
            Some(sig @ ('-' | '+')) => {
                let sig = if sig == '-' { -1 } else { 1 };
                let unit = match time_s.chars().last()? {
                    's' => 1,
                    'm' => 60,
                    'h' => 60 * 60,
                    'd' => 60 * 60 * 24,
                    'y' => 60 * 60 * 24 * 365,
                    _ => return None,
                };
                let n: i64 = time_s[1..(time_s.len() - 1)].trim_end().parse().ok()?;
                Some(now + (sig * n * unit))
            }
            Some(_) => None,
        }
    } else {
        None
    }
}

fn main() {
    // we always work in UTC here...
    std::env::set_var("TZ", "UTC");

    if std::process::Command::new("which")
        .arg("curl")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()
        .and_then(|s| s.code())
        != Some(0)
    {
        eprintln!("error: curl is required to be in PATH");
    };

    let mut username = None;
    let mut token = None;
    let mut url = None;
    let mut from = None;
    let mut to = None;
    let mut interval = None;
    let mut debug = 0;
    let mut follow = false;
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            f @ ("-h" | "--help") => {
                usage(f == "-h");
                return;
            }
            "-v" | "-vv" | "-vvv" => debug += arg.as_str().len() - 1,
            "-u" | "--user" => username = args.next(),
            "-t" | "--token" => token = args.next(),
            "--from" => from = args.next(),
            "--to" => to = args.next(),
            "--interval" => interval = args.next(),
            "-f" => follow = true,
            flag if flag.starts_with("-") => {
                eprintln!("error: unknown flag {flag:?}");
                usage(true);
                std::process::exit(1);
            }
            _ => url = Some(arg),
        }
    }

    let mut graf: Vec<&str> = vec![
        "-H",
        "Content-Type: application/json",
        "-H",
        "Accept: application/json",
    ];
    if debug > 2 {
        graf.push("-v");
    }
    let token = token.map(|t| format!("Authorization: Bearer {t}"));
    if let Some(ref token) = token {
        graf.extend(["-H", token]);
    } else if let Some(ref userpass) = username {
        graf.extend(["-u", userpass]);
    } else {
        eprintln!("error: either USER:PASS or TOKEN must be provided");
        usage(true);
        std::process::exit(1);
    }

    let url = match url {
        Some(ref url) => url,
        None => {
            eprintln!("error: URL must be provided");
            usage(true);
            std::process::exit(1);
        }
    };
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap();
    const TS_ERRSTR: &'static str = "valid values for FROM/TO are condensed ISO8601 UTC datetime '20160201T130405', grafana relative 'now-5m', or unix epoch '1678864718'";
    let from = from.as_ref().map(String::as_str).unwrap_or("now-5m");
    let mut from = match parse_instant(from, now) {
        Some(time) => time,
        None => {
            eprintln!("error: {TS_ERRSTR}");
            return;
        }
    };
    let to = to.as_ref().map(String::as_str).unwrap_or("now");
    if follow && to != "now" {
        eprintln!("error: -f is only supported for --to now, disabling follow");
        follow = false;
    }
    let mut to = match parse_instant(to, now) {
        Some(time) => time,
        None => {
            eprintln!("error: {TS_ERRSTR}");
            return;
        }
    };
    let interval = match interval.map(|s| i64::from_str_radix(&s, 10)) {
        Some(Ok(i)) => Some(i),
        Some(Err(_)) => {
            eprintln!("error: SECS must be a number");
            usage(true);
            return;
        }
        None => None,
    };

    macro_rules! graf {
        ($f:literal $(, $fargs:expr)* $(; $($arg:expr),* )?) => {{
            let args:  &[&str] = &[$($($arg),*)?];
            let urlarg = format!($f $(, $fargs)*);
            if debug > 1 {
                println!("-> get {graf:?} {urlarg:?} {args:?}");
            }
            let output = std::process::Command::new("curl")
                .args(&graf)
                .arg(&urlarg)
                .args(args)
                .output()
                .expect(&url);

            let json = serde_json::from_slice(&output.stdout);
            match json {
                Ok(json) => {
                    if debug > 2 {
                        println!("<- json: {json}");
                        if !output.stderr.is_empty() {
                            use std::io::Write as _;
                            std::io::stderr().write(&output.stderr).unwrap();
                        }
                    }
                    Value(json)
                },
                Err(err) => {
                    eprintln!("json ({url}): {err}");
                    use std::io::Write as _;
                    let mut stderr = std::io::stderr().lock();
                    if debug > 2 {
                        stderr.write(b"<- text: \"").unwrap();
                        stderr.write(&output.stdout).unwrap();
                        stderr.write(b"\"\n").unwrap();
                    }
                    stderr.write(&output.stderr).unwrap();
                    std::process::exit(1);
                }
            }
        }};
    }

    fn prompt<'v>(select_a: &str, vals: &'v [Value], keys: &[&str], buf: &mut String) -> &'v Value {
        use std::ops::Index as _;
        buf.clear();
        if vals.len() == 1 {
            return vals.index(0);
        }
        for (i, v) in vals.into_iter().enumerate() {
            print!("{i} -");
            for key in keys {
                print!(" {key}={}", v[key].0.to_string());
            }
            println!();
        }
        loop {
            print!("Please select {select_a}: ");
            std::io::stdout().flush().unwrap();
            std::io::stdin().read_line(buf).unwrap();
            match buf.trim().parse::<usize>() {
                Ok(i) => return vals.index(i),
                Err(_) => {}
            }
        }
    }
    let mut input = String::new();
    let mut prompt = move |select_a, vals, keys| prompt(select_a, vals, keys, &mut input);

    fn get_termsz() -> (u16, u16) {
        let mut winsz = libc::winsize {
            ws_col: 10,
            ws_row: 10,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: just ffi
        let ret = unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut winsz as *mut _) };
        assert_eq!(ret, 0, "ioctl");
        (winsz.ws_row, winsz.ws_col)
    }

    let (rows, cols) = get_termsz();
    if debug > 1 {
        println!("rows:{rows} cols:{cols}");
    }
    let interval = interval.unwrap_or_else(|| (to - from) / i64::from(rows));

    let pageres = graf!("{url}/api/search?type=dash-db");
    let dash = prompt("a dashboard", pageres.a(), &["title"]);
    let dashuid = dash["uid"].s();
    let dash = graf!("{url}/api/dashboards/uid/{dashuid}");
    let panels = &dash["dashboard"]["panels"];
    let panel = prompt("a panel", panels.a(), &["title"]);
    let target = prompt("a target", panel["targets"].a(), &["refId"]);

    let refid = target["refId"].s();
    let mut query = target.0.clone();
    {
        let query = query.as_object_mut().unwrap();
        query.insert("maxDataPoints".to_string(), rows.into());
        query.insert("intervalMs".to_string(), (interval * 1000).into());
    }
    let get_values = |from: i64, to: i64| {
        let qarg = serde_json::Value::Object(serde_json::Map::from_iter([
            (
                "queries".to_string(),
                serde_json::Value::Array(vec![query.clone()]),
            ),
            ("from".to_string(), (from * 1000).to_string().into()),
            ("to".to_string(), (to * 1000).to_string().into()),
        ]))
        .to_string();
        if debug > 0 {
            println!("query: {}", qarg);
        }
        graf!("{url}/api/ds/query"; "-d", &qarg)
    };
    let parse_values = |vals: &Value| {
        let times: Vec<_> = vals[0]["data"]["values"][0]
            .a()
            .into_iter()
            .map(|v| v.i())
            .collect();
        let vals: Vec<Vec<_>> = vals
            .a()
            .into_iter()
            .map(|v| {
                v["data"]["values"][1]
                    .a()
                    .into_iter()
                    .map(|v| v.f())
                    .collect()
            })
            .collect();
        (times, vals)
    };

    let dsquery = get_values(from, to);
    let vals = &dsquery["results"][refid]["frames"];
    if vals.a().is_empty() {
        println!("no data");
        return;
    }
    let (mut times, vals) = parse_values(vals);

    let min = vals
        .iter()
        .flat_map(|vs| vs.iter())
        .fold(f64::INFINITY, |acc, &x| if x < acc { x } else { acc });
    let max = vals
        .iter()
        .flat_map(|vs| vs.iter())
        .fold(-f64::INFINITY, |acc, &x| if x > acc { x } else { acc });
    // make room for time stamps "13:04:05 "
    let cols = cols - 9;
    if debug > 1 {
        let log_base = (max - min).log10();
        println!("log_base:{log_base} min:{min} max:{max} cols:{cols}");
    }

    let scale = f64::from(cols - 1) / (max - min);
    let scale = |vals: Vec<Vec<f64>>| -> Vec<Vec<_>> {
        let scale = |v: f64| {
            let v = (v - min) * scale;
            if v.is_finite() && v >= 0.0 && v < f64::from(cols) {
                // SAFETY: finite (and not NaN), and fits in u16 asserted above
                Some(unsafe { v.to_int_unchecked::<u16>() })
            } else {
                None
            }
        };
        vals.into_iter()
            .map(|vs| vs.into_iter().map(scale).collect())
            .collect()
    };
    let mut scaled_vals: Vec<Vec<_>> = scale(vals);

    let colors = [31, 32, 33, 34, 35, 36];
    let mut i0 = 0;
    loop {
        for i in 1..scaled_vals[0].len() {
            let mut hdr = vec![];
            if (i0 + i) % usize::from(rows) == 1 {
                use std::io::Write as _;
                let step = (max - min) / f64::from(cols) * 16.0;
                for j in 0..=((cols + 1) / 16) {
                    write!(hdr, " {:<15.2}", min + step * f64::from(j)).expect("write header");
                }
            }
            if (i0 + i) % 5 == 1 {
                let mut buf = [b' '; 9];
                let time_s = timestamp(times[i], &mut buf);
                print!("{time_s} ");
            } else {
                print!("         ");
            }
            let mut hdr = hdr.into_iter();
            for j in 0..cols {
                let s = scaled_vals.iter().enumerate();
                let acc = hdr
                    .next()
                    .filter(|b| *b != b' ')
                    .map(|b| (90, char::from(b)));
                let fold = |k, vs: &[_]| {
                    let (x, xp) = match (vs[i], vs[i - 1]) {
                        (Some(x), Some(xp)) => (x, xp),
                        _ => return None,
                    };
                    if (x < j && j < xp) || (xp < j && j < x) {
                        Some((colors[k % colors.len()], '-'))
                    } else if x == j && xp == j {
                        Some((colors[k % colors.len()], '|'))
                    } else if x == j {
                        Some((colors[k % colors.len()], '.'))
                    } else if xp == j {
                        Some((colors[k % colors.len()], '\''))
                    } else {
                        None
                    }
                };
                let s = s
                    .fold(acc, |acc, (k, vs)| acc.or_else(|| fold(k, vs)))
                    .unwrap_or_else(|| if j % 16 == 0 { (90, '|') } else { (0, ' ') });
                if s.0 == 0 {
                    print!("{}", s.1);
                } else {
                    print!("\x1b[{}m{}\x1b[0m", s.0, s.1);
                }
            }
            println!();
        }

        if !follow {
            return;
        }

        i0 += scaled_vals.len() - 1;
        from = to - interval;
        loop {
            to += interval;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let now = i64::try_from(now).unwrap();
            while now > to + interval {
                to += interval;
            }
            if now < to {
                let lag = u64::try_from(to - now).unwrap();
                std::thread::sleep(std::time::Duration::from_secs(lag));
            }
            let dsquery = get_values(from, to);
            let vals = &dsquery["results"][refid]["frames"];
            if vals.a().is_empty() {
                println!("no data");
                continue;
            }
            let (times0, vals) = parse_values(vals);
            times = times0;
            scaled_vals = scale(vals);
            break;
        }
    }
}
