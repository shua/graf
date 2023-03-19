## USAGE

```
graf [-h|--help] <-u USER:PASS|-t TOKEN> URL [--from FROM] [--to TO] [--interval SECS] [-f]

  select and print grafana dashboard panel to terminal

  -u USER:PASS basic user password authentication
  -t TOKEN     api token
  URL          grafana base url
  --from FROM, --to TO
               time specifiers for grafana (defaults to now-1m, now)
  INTERVAL    interval in seconds between frames (defaults to <terminal rows> / TO-FROM)
  -f           follow, update data every INTERVAL seconds
```

## EXAMPLE

[![asciicast](https://asciinema.org/a/568183.svg)](https://asciinema.org/a/568183)

<!-- -------------- offline example (no pretty colours for you) -----------------

$ graf http://localhost:3000 -u admin:admin --from now-1m -f
0 - title="dash" uid="2qZKhw-4z"
1 - title="Simple Streaming Example" uid="TXSTREZ"
Please select a dashboard: 0
0 - title="Random Walk"
1 - title="flight sim"
2 - title="Simple dummy streaming example"
Please select a panel: 1
02:47:40 |-122.43        |-31.46----|-----59.52-----------150.50----------241.48------'  |332.45---'
         |               |    '-----|    |               |               |               | '.
         |               |          |-----.              |               |               |  |
         |               |          |    |'------.       |               |               |  '.
         |               |          |    |       '-----. |               |               |   '.
02:47:45 |               |          |    |             '-----.           |               |    '.
         |               |          |    |               |   '-----.     |               |     '.
         |               |          |    |               |         '------.              |      '.
         |               |          |    |               |               |'-----.        |       '.
         |               |          |    |               |               |      '-----.  |        '.
02:47:50 |               |    .-----|-------------------------------------------------'  | .-------'
         |               |    '-----|    |               |               |               | '.
         |               |          |-----.              |               |               |  |
         |               |          |    |'------.       |               |               |  '.
         |               |          |    |       '-----. |               |               |   '.
02:47:55 |               |          |    |             '-----.           |               |    '.
         |               |          |    |               |   '-----.     |               |     '.
         |               |          |    |               |         '------.              |      '.
         |               |          |    |               |               |'-----.        |       '.
^C
$

-->
