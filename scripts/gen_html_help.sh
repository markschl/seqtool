#!/bin/sh

wiki=../seqtool.wiki
html=../seqtool-doc

# generate local HTML docs

cnv_links() {
  sed -E 's_<a href="(wiki\/[^"#]+|[^"\/#]+)(#[^"])?"_<a href="\1.html\2"_g' $1
}

mkdir -p $html $html/wiki
pandoc --self-contained -s -c doc/pandoc.css README.md | cnv_links > $html/README.html
for f in $wiki/*.md; do
  name="$(basename ${f%.*})"
  pandoc --self-contained -s -c doc/pandoc.css $f| cnv_links > $html/wiki/$name.html
done
