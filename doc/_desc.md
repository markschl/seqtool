
## Usage

```
seqtool <command> [<options>] [<files>...]
```

All commands accept one or multiple files, or STDIN input. The output is written
to STDOUT or a file (`-o`, useful for [format conversion](wiki/pass)). Commands can
be easily chained using the pipe.

### Options recognized by all commands

Use `seqtool -h` or [see here](wiki/opts) for a full list of options.

### Performance

Seqtool is very fast for most tasks, see [here for a comparison with other tools](wiki/performance).

[![Linux build status](https://travis-ci.org/markschl/seqtool.svg?branch=master)](https://ci.appveyor.com/project/markschl/seqtool)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/markschl/seqtool?svg=true)](https://ci.appveyor.com/project/markschl/seqtool)
