#!/bin/bash
set -eu -o pipefail


git init -q
git config commit.gpgsign false
git config core.autocrlf false
git config core.ignorecase false

while read -r pattern nomatch; do
  echo "$pattern" "$nomatch"
  echo "$pattern" > .gitignore
  git check-ignore -vn "$nomatch" 2>&1 || :
done <<EOF >>git-baseline.nmatch
*/\ XXX/\
*/\\ XXX/\
/*foo bar/foo
/*foo bar/bazfoo
foo*bar foo/baz/bar
/*foo.txt hello/foo.txt
bar/foo baz/bar/foo
*hello.txt hello.txt-and-then-some
*hello.txt goodbye.txt
*some/path/to/hello.txt some/path/to/hello.txt-and-then-some
*some/path/to/hello.txt some/other/path/to/hello.txt
*some/path/to/hello.txt a/bigger/some/path/to/hello.txt
abc?def abc/def
a*b*c abcd
abc*abc*abc abcabcabcabcabcabcabca
a[0-9]b a_b
a[!0-9]b a0b
a[!0-9]b a9b
[!-] -
a[^0-9]b a0b
a[^0-9]b a9b
[^-] -
{a,b} a
{a,b} b
{[}],foo} }
{foo} foo
{*.foo,*.bar,*.wat} test.foo
{*.foo,*.bar,*.wat} test.bar
{*.foo,*.bar,*.wat} test.wat
abc*def abc/def
\[a-z] \a
\? \a
\* \\
aBcDeFg abcdefg
aBcDeFg ABCDEFG
aBcDeFg AbCdEfG
some/**/needle.txt some/other/notthis.txt
some/**/**/needle.txt some/other/notthis.txt
/**/test one/notthis
/**/test notthis
**/.* ab.c
**/.* abc/ab.c
.*/** a.bc
.*/** abc/a.bc
./foo foo
**/foo foofoo
**/foo/bar foofoo/bar
/*.c mozilla-sha1/sha1.c
**/m4/ltoptions.m4 csharp/src/packages/repositories.config
some/*/needle.txt some/needle.txt
some/*/needle.txt some/one/two/needle.txt
some/*/needle.txt some/one/two/three/needle.txt
.*/** .abc
foo/** foo
{**/src/**,foo} abc/src/bar
{**/src/**,foo} foo
abc[/]def abc/def
EOF

while read -r pattern match; do
  echo "$pattern" "$match"
  echo "$pattern" > .gitignore
  git check-ignore -vn "$match" 2>&1 || :
done <<EOF >>git-baseline.match
/*foo.txt barfoo.txt
*foo.txt bar/foo.txt
*.c mozilla-sha1/sha1.c
*.rs .rs
*hello.txt hello.txt
*hello.txt gareth_says_hello.txt
*hello.txt some/path/to/hello.txt
/*foo.txt foo.txt
*hello.txt some\path\to\hello.txt
*hello.txt an/absolute/path/to/hello.txt
*some/path/to/hello.txt some/path/to/hello.txt
a foo/a
a a
a*b a_b
a*b*c abc
a*b*c a_b_c
a*b*c a___b___c
abc*abc*abc abcabcabcabcabcabcabc
a*a*a*a*a*a*a*a*a aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
a*b[xyz]c*d abxcdbxcddd
☃ ☃
** abcde
** .asdf
** x/.asdf
a[0-9]b a0b
a[0-9]b a9b
a[!0-9]b a_b
[a-z123] 1
[1a-z23] 1
[123a-z] 1
[abc-] -
[-abc] -
[-a-c] b
[a-c-] b
[-] -
a[^0-9]b a_b
_[[]_[]]_[?]_[*]_!_ _[_]_?_*_!_
a,b a,b
\[ [
\? ?
\* *
aBcDeFg aBcDeFg
some/**/needle.txt some/needle.txt
some/**/needle.txt some/one/needle.txt
some/**/needle.txt some/one/two/needle.txt
some/**/needle.txt some/other/needle.txt
some/**/**/needle.txt some/needle.txt
some/**/**/needle.txt some/one/needle.txt
some/**/**/needle.txt some/one/two/needle.txt
some/**/**/needle.txt some/other/needle.txt
**/test one/two/test
**/test one/test
**/test test
/**/test one/two/test
/**/test one/test
/**/test test
**/.* .abc
**/.* abc/.abc
**/foo/bar foo/bar
.*/** .abc/abc
test/** test/
test/** test/one
test/** test/one/two
some/*/needle.txt some/one/needle.txt
abc/def abc/def
EOF

git config core.ignorecase true
while read -r pattern match; do
  echo "$pattern" "$match"
  echo "$pattern" > .gitignore
  git check-ignore -vn "$match" 2>&1 || :
done <<EOF >>git-baseline.match-icase
aBcDeFg  aBcDeFg
aBcDeFg  abcdefg
aBcDeFg  ABCDEFG
aBcDeFg  AbCdEfG
EOF

# nmatches OS specific
# windows
#    "abc?def" "abc\\def"
# unix
#    "abc\\def" "abc/def"


# matches OS specific

# unix only
# "\\a"  "a"
#"abc\\def"  "abc/def"
#"abc?def"  "abc/def"

# windows only
# "abc[/]def" "abc/def"
# "abc\\def"  "abc/def"
#"abc?def"  "abc\\def"

# empty string is not a valid path-spec
#** " "
#{} " "
#{,} " "
