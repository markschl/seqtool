#!/bin/sh

wiki=../seqtool-wiki
html=../seqtool-doc

# generate local HTML docs

cnv_links() {
  sed -E 's#<a href="(wiki/[^"]+|[^"/]+)"#<a href="\1.html"#g' $1
}

mkdir -p $html
pandoc --self-contained -s -c doc/pandoc.css README.md | cnv_links > $html/index.html
for f in $wiki/*.md; do
  name="$(basename ${f%.*})"
  pandoc --self-contained -s -c doc/pandoc.css $f| cnv_links > $html/$name.html
done
