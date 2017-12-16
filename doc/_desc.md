
## Usage

```
seqtool <command> [<options>] [<files>...]
```

All commands accept one or multiple files, or STDIN input. The output is written
to STDOUT or a file (`-o`, useful for [format conversion](wiki/pass)). Commands can
be easily chained using the pipe.

Use `seqtool <command> -h` to see all available options. A full list of options
that are accepted by all commands can be [found here](wiki/opts).

## Installing

Binaries for Linux, Mac OS X and Windows can be
[downloaded from the releases section](https://github.com/markschl/seqtool/releases/latest).

## Performance

Seqtool is very fast for most tasks, see [here for a comparison with other tools](wiki/performance).
