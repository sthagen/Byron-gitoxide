# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.0 (2023-02-17)

<csr-id-f7f136dbe4f86e7dee1d54835c420ec07c96cd78/>
<csr-id-533e887e80c5f7ede8392884562e1c5ba56fb9a8/>

### Chore

 - <csr-id-f7f136dbe4f86e7dee1d54835c420ec07c96cd78/> uniformize deny attributes
 - <csr-id-533e887e80c5f7ede8392884562e1c5ba56fb9a8/> remove default link to cargo doc everywhere

### Bug Fixes

 - <csr-id-e14dc7d475373d2c266e84ff8f1826c68a34ab92/> note that crates have been renamed from `git-*` to `gix-*`.
   This also means that the `git-*` prefixed crates of the `gitoxide` project
   are effectively unmaintained.
   Use the crates with the `gix-*` prefix instead.
   
   If you were using `git-repository`, then `gix` is its substitute.

### New Features (BREAKING)

 - <csr-id-3d8fa8fef9800b1576beab8a5bc39b821157a5ed/> upgrade edition to 2021 in most crates.
   MSRV for this is 1.56, and we are now at 1.60 so should be compatible.
   This isn't more than a patch release as it should break nobody
   who is adhering to the MSRV, but let's be careful and mark it
   breaking.
   
   Note that `git-features` and `git-pack` are still on edition 2018
   as they make use of a workaround to support (safe) mutable access
   to non-overlapping entries in a slice which doesn't work anymore
   in edition 2021.

## v0.0.0 (2025-01-18)

### New Features (BREAKING)

 - <csr-id-787cf6f5a838a96da49330c99a8530ac3206de50/> add `range` to `blame::file()`

### New Features

 - <csr-id-4ffe6eb8f7921c6a03db0aa6d796cc2e3cc328e0/> Add support for statistics and additional performance information.
 - <csr-id-25efbfb72e5a043ce8f7d196c1f7104ef93394df/> Add `blame` plumbing crate to the top-level.
   For now, it doesn't come with a simplified `gix` API though.
 - <csr-id-17835bccb066bbc47cc137e8ec5d9fe7d5665af0/> bump `rust-version` to 1.70
   That way clippy will allow to use the fantastic `Option::is_some_and()`
   and friends.
 - <csr-id-64ff0a77062d35add1a2dd422bb61075647d1a36/> Update gitoxide repository URLs

### Chore

 - <csr-id-17835bccb066bbc47cc137e8ec5d9fe7d5665af0/> bump `rust-version` to 1.70
   That way clippy will allow to use the fantastic `Option::is_some_and()`
   and friends.

### Other

 - <csr-id-64ff0a77062d35add1a2dd422bb61075647d1a36/> Update gitoxide repository URLs
   This updates `Byron/gitoxide` URLs to `GitoxideLabs/gitoxide` in:
   
   - Markdown documentation, except changelogs and other such files
     where such changes should not be made.
   
   - Documentation comments (in .rs files).
   
   - Manifest (.toml) files, for the value of the `repository` key.
   
   - The comments appearing at the top of a sample hook that contains
     a repository URL as an example.
   
   When making these changes, I also allowed my editor to remove
   trailing whitespace in any lines in files already being edited
   (since, in this case, there was no disadvantage to allowing this).
   
   The gitoxide repository URL changed when the repository was moved
   into the recently created GitHub organization `GitoxideLabs`, as
   detailed in #1406. Please note that, although I believe updating
   the URLs to their new canonical values is useful, this is not
   needed to fix any broken links, since `Byron/gitoxide` URLs
   redirect (and hopefully will always redirect) to the coresponding
   `GitoxideLabs/gitoxide` URLs.
   
   While this change should not break any URLs, some affected URLs
   were already broken. This updates them, but they are still broken.
   They will be fixed in a subsequent commit.
   
   This also does not update `Byron/gitoxide` URLs in test fixtures
   or test cases, nor in the `Makefile`. (It may make sense to change
   some of those too, but it is not really a documentation change.)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 46 commits contributed to the release.
 - 5 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Don't specify version numbers in dev-dependencies ([`7570daa`](https://github.com/GitoxideLabs/gitoxide/commit/7570daa50a93a2b99e9cd5228cb274f20839865f))
    - Update all changelogs prior to release ([`1f6390c`](https://github.com/GitoxideLabs/gitoxide/commit/1f6390c53ba68ce203ae59eb3545e2631dd8a106))
    - Merge pull request #1766 from cruessler/add-range-to-gix-blame ([`90fef01`](https://github.com/GitoxideLabs/gitoxide/commit/90fef0148376167763a3ebeff91a1cf9c236cf8a))
    - Refactor ([`1500c08`](https://github.com/GitoxideLabs/gitoxide/commit/1500c08736069153aab33842d2d877f42ad01f37))
    - Add `range` to `blame::file()` ([`787cf6f`](https://github.com/GitoxideLabs/gitoxide/commit/787cf6f5a838a96da49330c99a8530ac3206de50))
    - Merge pull request #1762 from GitoxideLabs/fix-1759 ([`7ec21bb`](https://github.com/GitoxideLabs/gitoxide/commit/7ec21bb96ce05b29dde74b2efdf22b6e43189aab))
    - Bump `rust-version` to 1.70 ([`17835bc`](https://github.com/GitoxideLabs/gitoxide/commit/17835bccb066bbc47cc137e8ec5d9fe7d5665af0))
    - Merge pull request #1756 from cruessler/extract-object-ids-in-tests ([`f18a312`](https://github.com/GitoxideLabs/gitoxide/commit/f18a3129b11c53e7922295908a6930039b8203c3))
    - Extract hard-coded ObjectIds in tests ([`50ba3d6`](https://github.com/GitoxideLabs/gitoxide/commit/50ba3d6aa60a67cbacb2aa7411e3f20c3c6cf0c0))
    - Merge pull request #1755 from cruessler/shortcut-tree-diffing-minor-cleanups ([`25c2646`](https://github.com/GitoxideLabs/gitoxide/commit/25c2646f2c7f0430791fc14131a7e103f3c9cac7))
    - Prefix variant to disambiguate from continue ([`ec3cdf1`](https://github.com/GitoxideLabs/gitoxide/commit/ec3cdf1520837db9a94257db3b08099e34892baa))
    - Merge pull request #1754 from GitoxideLabs/fix-ci ([`34096a5`](https://github.com/GitoxideLabs/gitoxide/commit/34096a5796f03f76e8ed696b886fbd62eb09d2cc))
    - Fix clippy ([`6805beb`](https://github.com/GitoxideLabs/gitoxide/commit/6805beb31609bff9dad1807901d8901024ab1d3c))
    - Merge pull request #1753 from GitoxideLabs/wip-changes-against-more-than-one-parent ([`a22f13b`](https://github.com/GitoxideLabs/gitoxide/commit/a22f13bec0cdd580ee92390a98d5d522eb29978d))
    - Refactor ([`360bf38`](https://github.com/GitoxideLabs/gitoxide/commit/360bf383a3ebdeeda1db161d42bb057a05cdf32b))
    - Rework how blame is passed to parents ([`a3d92b4`](https://github.com/GitoxideLabs/gitoxide/commit/a3d92b4d1f129b18217d789273c4991964891de0))
    - Merge pull request #1747 from cruessler/shortcut-tree-diffing ([`59bd978`](https://github.com/GitoxideLabs/gitoxide/commit/59bd978ba560295ed4fcb86f1a629e3c728dd5dd))
    - Update doc-string ([`9ac36bd`](https://github.com/GitoxideLabs/gitoxide/commit/9ac36bdd0af860df24c303d0d4a789b324ab2c43))
    - Rename to FindChangeToPath and move to where it's used ([`f857ca8`](https://github.com/GitoxideLabs/gitoxide/commit/f857ca86f88b25dc1ce1ca7c90db05793828ddf0))
    - Simplify Recorder by wrapping gix_diff::tree::Recorder ([`7d1416a`](https://github.com/GitoxideLabs/gitoxide/commit/7d1416a9124c16e757a3e7cb3fd762c9e52973bb))
    - Don't ignore gix_diff::tree errors ([`f049b00`](https://github.com/GitoxideLabs/gitoxide/commit/f049b00b9d59b3eff4c9489557d9d709f96fdd67))
    - Cancel tree diffing early when matching path is found ([`74565bc`](https://github.com/GitoxideLabs/gitoxide/commit/74565bc2c5ab46348a0e9182e7b9d946dfbc0dd8))
    - Merge pull request #1453 from cruessler/gix-blame ([`6ed9976`](https://github.com/GitoxideLabs/gitoxide/commit/6ed9976abaa3915b50efa46c46b195f3a1fc4ff7))
    - For linear histories, avoid redoing path lookup work ([`8196a43`](https://github.com/GitoxideLabs/gitoxide/commit/8196a433ed08de6b09b5cb187f8ce53fc2ab09ca))
    - Don't panic when suspect isn't known when converting unblamed to blame-entry ([`667e626`](https://github.com/GitoxideLabs/gitoxide/commit/667e6262bcba1d95e32795faa79dc6b354da9a01))
    - Additional pass of refactoring, focus on the algorithm itself. ([`3ac8be1`](https://github.com/GitoxideLabs/gitoxide/commit/3ac8be1557de8a66ff32abe3d1c9ea83198d4a05))
    - Review and remove all TODOs where possible, update docs and comments ([`63ee0f9`](https://github.com/GitoxideLabs/gitoxide/commit/63ee0f9c34dc89ad51d5c9ab83e49cbc08e3ed69))
    - Swap blamed-file and original-file variable names. ([`b7f1468`](https://github.com/GitoxideLabs/gitoxide/commit/b7f1468f0fe38a50ad3414efb5efcf3ac0d2fddb))
    - Replace todos!() with assertions or remove them. ([`b736ace`](https://github.com/GitoxideLabs/gitoxide/commit/b736ace18e8996b410a597fb4f43bf28f422dfc5))
    - Add `Error` type ([`845d96a`](https://github.com/GitoxideLabs/gitoxide/commit/845d96a4ffff89703a8c3815ac52adc7f2b286f6))
    - Add support for statistics and additional performance information. ([`4ffe6eb`](https://github.com/GitoxideLabs/gitoxide/commit/4ffe6eb8f7921c6a03db0aa6d796cc2e3cc328e0))
    - Remove duplication and unnecessary parameter ([`a158d22`](https://github.com/GitoxideLabs/gitoxide/commit/a158d22703077d37b83e0434aa229baf12c342ed))
    - Unify how lines in blame results are accessed ([`f2790a9`](https://github.com/GitoxideLabs/gitoxide/commit/f2790a9db8cac3ce57003b512edf735e734383d1))
    - Modularlize `gix-blame/lib.rs` ([`26bfd2d`](https://github.com/GitoxideLabs/gitoxide/commit/26bfd2d73374e134aff24410fac44857b8128244))
    - First review round ([`983ec7d`](https://github.com/GitoxideLabs/gitoxide/commit/983ec7d776b459898b90927242582fc03a0e9056))
    - Add `blame` plumbing crate to the top-level. ([`25efbfb`](https://github.com/GitoxideLabs/gitoxide/commit/25efbfb72e5a043ce8f7d196c1f7104ef93394df))
    - Add initial implementation and tests for `gix-blame`. ([`d27adf7`](https://github.com/GitoxideLabs/gitoxide/commit/d27adf70b4e2f57d8431a0a553119322d7158f4b))
    - Merge pull request #1624 from EliahKagan/update-repo-url ([`795962b`](https://github.com/GitoxideLabs/gitoxide/commit/795962b107d86f58b1f7c75006da256d19cc80ad))
    - Update gitoxide repository URLs ([`64ff0a7`](https://github.com/GitoxideLabs/gitoxide/commit/64ff0a77062d35add1a2dd422bb61075647d1a36))
    - Merge pull request #1589 from EliahKagan/maintenance ([`7c2af44`](https://github.com/GitoxideLabs/gitoxide/commit/7c2af442748f7245734ec1f987b6d839f2a795bd))
    - Add missing executable bits ([`694ebad`](https://github.com/GitoxideLabs/gitoxide/commit/694ebadb2d11d25c5b1285c61cef5df03685701a))
    - Merge branch 'global-lints' ([`37ba461`](https://github.com/GitoxideLabs/gitoxide/commit/37ba4619396974ec9cc41d1e882ac5efaf3816db))
    - Workspace Clippy lint management ([`2e0ce50`](https://github.com/GitoxideLabs/gitoxide/commit/2e0ce506968c112b215ca0056bd2742e7235df48))
    - Merge branch 'gix-blame' ([`e6fbea9`](https://github.com/GitoxideLabs/gitoxide/commit/e6fbea9be2ef7ab4064dc57c8233dfe81fac3bb4))
    - Add sample fixture ([`6d71e0d`](https://github.com/GitoxideLabs/gitoxide/commit/6d71e0d291f2a3b11c635949712ec86cf57d7449))
    - Add new `gix-blame` crate ([`f5f616d`](https://github.com/GitoxideLabs/gitoxide/commit/f5f616d8345898effc79d587c139e249f1c85ab6))
</details>

