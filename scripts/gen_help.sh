#!/bin/bash

# cargo build

seqtool=target/debug/st
outdir=../seqtool-docs
main=../seqtool-docs/index.md #_README.md
nav=../seqtool-docs/nav.yml

# # prepend table of contents if there are H3 headings
# prepend_toc() {
#   contents="$1"
#   level="$2"
#   toc_level="$3"
#   toc=`grep "^$level " "$contents" |
#     sed -E "s/^$level (.*)/* [\1]   #\1/g" |
#     awk -F'   ' '{ gsub(" ", "-", $2); rep=gsub("[()]", "", $2); print sprintf("%s(%s)", $1, tolower($2)) }'`

#   if [ `printf "$toc" | wc -l` -gt 1 ]; then
#     printf "$toc_level Contents\n\n$toc\n\n" | cat - "$contents" > tmp_out
#     mv tmp_out "$contents"
#   fi
# }


# echo -e "---\npermalink: /\ntitle: title\nwide: true\nsidebar:\n  \nnav: docs\n---\n" > tmp_out

echo -e "docs:\n  - title: Commands\n    children:\n" > $nav

cat doc/_head.md > $main

# generate command files

printf "\n## Commands" >> $main

cmd=(
  ">Basic conversion / editing" pass
  ">Information about sequences" view count stat
  ">Subsetting/shuffling sequences" sort unique filter split sample slice head tail interleave
  ">Searching and replacing" find replace
  ">Modifying commands" del set trim mask upper lower revcomp concat
)

# create one MD file per command

for c in "${cmd[@]}"; do
  echo "$c"

  if [[ "$c" = ">"* ]]; then
    # category name
    c=$(echo "$c" | cut -c2-)
    printf "\n### $c\n" >> $main
    continue
  fi

  out=$outdir/$c.md
  echo -n > $out

  opts=$(stty cols 80 && $seqtool "$c" -h 2>&1 | sed -n '/General options/q;p')
  desc=$(echo "$opts" | sed -n '/^ *$/q;p')

  # add command to overview
  echo "* **[$c](https://markschl.github.io/seqtool/$c)**: $desc" >> $main

  # add custom help content if file exists in doc dir
  desc_f=doc/$c.md
  if [ -f $desc_f ]; then
    echo "## Details" >> $out
    cat $desc_f >> $out
  fi

  # add variable help if present
  vars=$($seqtool $c  --help-vars-md --help-cmd-vars 2>&1 || true)
  if [ ! -z "$vars" -a "$vars" != " "  ]; then
    echo -e "$vars" | sed 's/^ *#/##/g' >> $out
  fi

  # prepend_toc $out '###' '##'

  # TODO: why prepend?
  # prepend usage info
  # echo -e "---\npermalink: /$c/\ntitle: $c\ntoc: true\nsidebar:\n  nav: docs\n---\n" > tmp_out
  usage=$(echo "$opts" | sed '/Usage:/,$!d')
  printf "$desc\n\n\`\`\`\n$usage\n\`\`\`\n\n[See this page](opts) for the options common to all commands.\n\n" |
    cat - $out >> tmp_out
    mv tmp_out $out

  echo -e "      - title: $c\n        url: /$c" >> $nav

done


echo >> $main
cat doc/_desc.md >> $main

# variables/functions
out=$outdir/variables.md
cp doc/variables.md $outdir/variables.md
# full variables reference
out=$outdir/var_reference.md
$seqtool . --help-vars-md 2>&1 > $out
prepend_toc $out '##' '##'
mv $out tmp_out
echo -e "\n# Variables/functions: full reference\n" > $out
cat tmp_out >> $out
rm tmp_out

# args common to all commands
out=$outdir/opts.md
printf "\n\n### Options recognized by all commands\n\n" > $out
echo "\`\`\`" >> $out
stty cols 80 && $seqtool pass -h 2>1 | sed '/General options/,$!d' >> $out
echo "\`\`\`" >> $out

# other files

# TODO: doc/expressions.md
cp doc/meta.md doc/ranges.md doc/attributes.md $outdir
