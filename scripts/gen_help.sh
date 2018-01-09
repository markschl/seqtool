#!/bin/bash

cargo build --features=exprtk

seqtool=target/debug/seqtool
wiki=../seqtool.wiki
main=_README.md

cat doc/_head.md > $main

# generate command files

printf "\n## Commands" >> $main

cmd=(
  ">Basic conversion / editing" pass
  ">Information about sequences" count stat
  ">Subsetting/shuffling sequences" head tail slice sample filter split
  ">Searching and replacing" find replace
  ">Modifying commands" del set trim mask upper lower revcomp
)
for c in "${cmd[@]}"; do
  echo "$c"

  if [[ "$c" = ">"* ]]; then
    # category name
    c=$(echo "$c" | cut -c2-)
    printf "\n### $c\n" >> $main
    continue
  fi

  out=$wiki/$c.md
  opts=$($seqtool "$c" -h 2>&1 | sed -n '/Input options/q;p')
  desc=$(echo "$opts" | sed -n '/Usage:/q;p')
  usage=$(echo "$opts" | sed '/Usage:/,$!d' | sed 's/\[-p <prop>\.\.\.\] *\[-l <list>\.\.\.\]//g')
  printf "$desc\n\n" > $out
  printf "\`\`\`\n$usage\n\`\`\`\n\n" >> $out
  printf "[See this page](opts) for the options common to all commands.\n\n" >> $out

  # add to overview
  echo "* **[$c](wiki/$c)**: $desc" >> $main

  # add custom descriptions
  if [ -f doc/$c.md ]; then
    echo "## Description" >> $out
    cat doc/$c.md >> $out
  fi

  # variable help
  vars=$($seqtool $c --help-vars 2>&1 | sed -n '/Standard variables/q;p' )
  if [ ! -z "$vars" -a "$vars" != " "  ] && [[ "$vars" != Invalid* ]]; then
    printf "\n\n### Provided variables\n\`\`\`\n$vars\n\`\`\`\n\n" >> $out
  fi
done

cat doc/_desc.md >> $main

# variables
out=$wiki/variables.md
cat doc/variables.md > $out
printf "\n## Variables available to all commands\n\n\`\`\`\n" >> $out
$seqtool --help-vars >> $out 2>&1
echo "\`\`\`" >> $out

# global opts
out=$wiki/opts.md
printf "\n\n### Options recognized by all commands\n\n" > $out
echo "\`\`\`" >> $out
$seqtool . -h 2>&1 | sed '/Input options:/,$!d' >> $out
echo "\`\`\`" >> $out

# other files

cp doc/lists.md doc/ranges.md doc/attributes.md $wiki

# replace URLs in readme

sed -E 's|wiki/|https://github.com/markschl/seqtool/wiki/|g' $main > README.md
cp README.md $wiki/Home.md
rm $main
