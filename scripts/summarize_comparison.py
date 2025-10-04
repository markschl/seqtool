#!/usr/bin/env python3

import json
import re


def gen_summary(json_input, md_out):
    from html import escape
    # def cnv_breaks(s):
    #     return re.sub(r'[\r\n]', '<br/>', s, re.DOTALL)

    def fmt_bench(d):
        return '{}{}<br/>{}'.format(
            format_time(d),
            ' {:.0f}% CPU'.format(d['cpu']) if abs(100 - d['cpu']) > 5 else '',
            format_mem(d),
        )

    def find_best(d):
        runs = [d['st']] + list(d.get('other', {}).values())
        if len(runs) > 1:
            times = sorted((r['elapsed'], i) for i, r in enumerate(runs))
            if times[0][0] > 0:
                runs[times[0][1]]['fastest'] = times[1][0] / times[0][0]
            mem = sorted((r['max_mib'], i) for i, r in enumerate(runs))
            if mem[0][0] > 0:
                runs[mem[0][1]]['lowest_mem'] = mem[1][0] / mem[0][0]

    def format_time(d):
        f = d.get('fastest')
        if f is not None:
            return '🕓 <b>{:.1f} s</b> 🏆 ({:.1f}x)'.format(d['elapsed'], f)
        return '🕓 {:.1f} s'.format(d['elapsed'])

    def format_mem(d):
        f = d.get('lowest_mem')
        if f is not None:
            return '📈 <b>{:.1f} MiB</b> 🏆 ({:.2f}x)'.format(d['max_mib'], f)
        return '📈 {:.1f} MiB'.format(d['max_mib'])

    def fmt_output(d):
        strip_newlines = lambda msg: re.sub(r'(?ms:[\r\n\s]+$)', '', msg)
        out = ''
        if d['stdout']:
            # TODO: get this to work: https://github.com/squidfunk/mkdocs-material/issues/4964
            out += '<details><summary>🟦 output</summary>\n\n```\n{}\n```\n\n</details>\n'.format(strip_newlines(d['stdout']))
        if d['stderr']:
            out += '<details><summary> messages</summary>\n\n```\n{}\n```\n\n</details>\n'.format(strip_newlines(d['stderr']))
        return out

    for command, comparisons in json.load(json_input).items():
        md_out.write('## {}\n'.format(command))
        md_out.write('<table markdown class="cmd">\n\n')
        for comparison, d in comparisons.items():
            find_best(d)
            st = d['st']
            md_out.write('<tr markdown>\n<td markdown>\n\n{}\n\n</td>\n\n'.format(escape(d.get('description', comparison))))
            md_out.write('<td markdown>\n\n```bash\n{}\n```\n\n{}\n'.format(st['cmd'], fmt_output(st)))
            if 'other' in d and d['other']:
                md_out.write('<details markdown><summary>{}</summary>\n\n<table markdown class="cmd">\n'.format(
                    "  ❙  ".join('<b>{}</b> {}'.format(
                        escape(tool), format_time(o),
                    )
                    for tool, o in d['other'].items())
                )
                )
                for tool, o in d['other'].items():
                    code = '<td markdown>\n\n```bash\n{}\n```\n\n{}</td>'.format(
                        o['cmd'].replace('\n', ' '),
                        fmt_output(o)
                    )
                    md_out.write('\n<tr markdown>\n<td markdown>{}</td>\n\n{}\n\n<td markdown>{}</td>\n\n</tr>\n\n'.format(
                        escape(tool),
                        code,
                        fmt_bench(o)
                    ))
                md_out.write('</table>\n\n</details>\n\n')
            md_out.write('</td>\n\n<td>{}</td>\n\n</tr>\n\n'.format(fmt_bench(st)))
        md_out.write('</table>\n\n')

if __name__ == '__main__':
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument('json_input', type=argparse.FileType('r'))
    parser.add_argument('md_out', type=argparse.FileType('w'))
    # parser.add_argument('-m', '--main-only', action='store_true')
    args = parser.parse_args()

    gen_summary(**vars(args))
