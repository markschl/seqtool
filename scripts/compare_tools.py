#!/usr/bin/env python3

import os


def run_benches(binary, input_dir, config, temp_dir=None, kind=None):
    assert os.path.basename(binary) == 'st', "The binary name must be 'st'"
    d = os.path.abspath(os.path.dirname(binary))
    os.environ['PATH'] = d + ":" + os.environ['PATH']
    # sys.path.insert(0, d)
    if temp_dir is not None:
        temp_dir = os.path.relpath(temp_dir, input_dir)
    os.chdir(input_dir)
    out = {}
    for cmd, benches in config.items():
        cmd_out = out[cmd] = {}
        for bench, cfg in benches.items():
            print("bench", bench)
            cmd_out[bench] = run_bench(bench, cfg, temp_dir=temp_dir, kind=kind)
    return out

def run_bench(name, cfg, temp_dir=None, kind=None):
    # print(cfg.get('description', name))
    # from pprint import pprint; pprint(cfg)
    st_cmd = cfg['cmd']
    # do time measurements
    out = {'description': cfg.get('description', name), 'other': {}}
    if 'prepare' in cfg:
        if isinstance(cfg['prepare'], str):
            cfg['prepare'] = [cfg['prepare']]
        for cmd in cfg['prepare']:
            call(cmd, shell=True)
    if kind is None or 'main' in kind:
        out['st'], outfile = run_command(st_cmd, temp_dir=temp_dir)
        if outfile is not None:
            os.remove(outfile)
    if kind is None or 'other' in kind:
        for what, cmd in cfg.get('other', {}).items():
            # print(what, cmd, len(cmd))
            out['other'][what], outfile = run_command(cmd, temp_dir=temp_dir)
            if outfile is not None:
                os.remove(outfile)
    # run comparisons
    if kind is None or 'comparisons' in kind:
        out1 = 'out1'
        out2 = 'out2'
        if 'compare_with' in cfg:
            _, outfile = run_command(st_cmd, temp_dir=temp_dir)
            os.rename(outfile, out1)
            for what in cfg['compare_with']:
                # print("compare", what)
                cmd = cfg.get('other', {})[what]
                _, outfile = run_command(cmd)
                os.rename(outfile, out2)
                assert_identical(out1, out2, st_cmd, cmd)
                os.remove(out2)
            os.remove(out1)
        for what, cmds in cfg.get('compare', {}).items():
            # print("compare other", what)
            assert len(cmds) == 2
            _, outfile = run_command(cmds[0], temp_dir=temp_dir)
            os.rename(outfile, out1)
            _, outfile = run_command(cmds[1], temp_dir=temp_dir)
            os.rename(outfile, out2)
            assert_identical(out1, out2, *cmds)
            os.remove(out1)
            os.remove(out2)
    if 'cleanup' in cfg:
        if isinstance(cfg['cleanup'], str):
            cfg['cleanup'] = [cfg['cleanup']]
            for cmd in cfg['cleanup']:
                call(cmd, shell=True)
    return out

def assert_identical(f1, f2, cmd1, cmd2):
    import subprocess
    try:
        subprocess.check_call(["diff", "-q", f1, f2])
    except subprocess.CalledProcessError:
        import sys
        print(
            "COMPARISON ERROR: command output differs for:\n- {}\n- {}\nCheck with diff {} {}".format(
                cmd1, cmd2, os.path.abspath(f1),
                os.path.abspath(f2), os.path.abspath(f2)),
                file=sys.stderr
            )


def run_command(command, temp_dir=None):
    import re
    from tempfile import mkstemp
    # look for an [[alternative command]] to use instead (appended to command that is displayed)
    mod_cmd = command
    m = re.search(r"^(.+?)\[\[(.+?)\]\] *$", mod_cmd)
    if m is not None:
        command, mod_cmd = m.groups()
    # look for individual args that should be used, but not be displayed by default
    command = re.sub(r' +\[.+?\]', ' ', command)
    mod_cmd = re.sub(r' +\[(.+?)\]', r' \1', mod_cmd)
    print(command)
    if command != mod_cmd:
        print("[[ {} ]]".format(mod_cmd))
    _, time_out = mkstemp(prefix="st_time", dir=temp_dir)
    # for running the command, we use Bash here
    _cmd = ['/usr/bin/time', '-o', time_out, '-f', r'%e\t%M\t%P', 'bash', '-c', mod_cmd]
    stdout, stderr = call(_cmd)
    with open(time_out) as f:
        # we just use the last line, since the first line will have an error message
        # if the command fails
        info = list(f.read().splitlines())[-1].split('\t')
    os.remove(time_out)
    result = {
        'cmd': command,
        'modified_cmd': None if mod_cmd == command else mod_cmd,
        'stdout': stdout.decode('utf-8'),
        'stderr': stderr.decode('utf-8'),
        'elapsed': float(info[0]),
        # https://stackoverflow.com/questions/61392725/kilobytes-or-kibibytes-in-gnu-time
        'max_mib': float(info[1])/1024.,
        'cpu': float(re.sub('%$', '', info[2])),
    }
    # Get output file name
    m = re.search(r'\b(output(\.\w+)+)( |$)', mod_cmd)
    outfile = None if m is None else m.group(1)
    return result, outfile


def call(cmd, **kwarg):
    import subprocess
    # print("call", cmd)
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kwarg)
    out, err = p.communicate()
    if p.returncode != 0:
        import sys
        print("ERROR: non-zero exit code {} for command:\n{}\nstderr:\n{}".format(p.returncode, cmd, err.decode('utf-8')), file=sys.stderr)
    return out, err


def generate_files(fastq_path, outdir):
    if not os.path.exists(outdir):
        os.makedirs(outdir)
    fq = os.path.join(outdir, 'input.fastq')
    if os.path.exists(fq):
        os.remove(fq)
    os.symlink(os.path.abspath(fastq_path), os.path.abspath(fq))
    fa = os.path.splitext(fq)[0] + '.fasta'
    if not os.path.exists(fa):
        call(['st', '.', '--to-fa', fq, '-o', fa])


if __name__ == '__main__':
    import argparse
    import yaml
    import json

    parser = argparse.ArgumentParser()
    parser.add_argument('fastq')
    parser.add_argument('configfile')
    parser.add_argument('-o', '--output', type=argparse.FileType('w'), default='-')
    parser.add_argument('-s', '--selection',
                        help='comma-separated list of benchmarks to run')
    parser.add_argument('-b', '--binary', default='target/release/st')
    parser.add_argument('-d', '--input-dir', default='target/st_benchmark')
    parser.add_argument('-t', '--temp-dir')
    parser.add_argument('-k', '--kind', default='main,other,comparisons',
                        help="Comma-separated list of benchmark types to run. "
                        "Possible are 'main' (the seqtool commands), 'other' (other tools) "
                        "and 'comparisons' (comparison of output from different tools). "
                        "Default is to run all.")
    args = parser.parse_args()

    generate_files(args.fastq, args.input_dir)

    with open(args.configfile) as f:
        config = yaml.safe_load(f)

    if args.selection:
        sel = [s.strip() for s in args.selection.split(',')]
        config = {k: v for k, v in config.items() if k in sel}

    # print(config)
    kind = [k.strip() for k in args.kind.split(',')]
    out = run_benches(args.binary, args.input_dir, config, temp_dir=args.temp_dir, kind=kind)

    json.dump(out, args.output, indent=2)
