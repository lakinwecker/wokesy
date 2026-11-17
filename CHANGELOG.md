# Changelog

Rendered by [git-cliff](https://git-cliff.org) from the conventional commits
behind each tag. `release` rewrites this file in full on every release, so an
edit made here is lost — edit the commit messages instead.

## v0.4.3 — 2026-11-17

### Tooling

- **deploy**: deploy production 0.4.2 ([dc8b30b](https://github.com/lakinwecker/wokesy/commit/dc8b30bbfb070144d31a653f9a08019db4a2b5c3))
- upgrade github workflow machines ([9db2878](https://github.com/lakinwecker/wokesy/commit/9db28784c0bd053a2e494dbf148830fe91397a36))

## v0.4.2 — 2026-11-17

### Tooling

- **deploy**: deploy production 0.4.1 ([675c4e6](https://github.com/lakinwecker/wokesy/commit/675c4e6b426cc2d6add6cf0f9742452c5f1b280a))

### Other

- fix release for macos ([e7ca598](https://github.com/lakinwecker/wokesy/commit/e7ca59892823474542f07e24e7bc46a7787f2b3a))

## v0.4.1 — 2026-11-17

### Tooling

- **deploy**: deploy production 0.4.0 ([100923f](https://github.com/lakinwecker/wokesy/commit/100923f2b7931a0515c450b3c4e856bc89d2acb4))

## v0.4.0 — 2026-11-15

### Features

- **status**: post every run's status in one wokesy status topic ([ac2ff9b](https://github.com/lakinwecker/wokesy/commit/ac2ff9b340fdb8ecaf7c75ad387ec0b5e19b5968))
- **zulip**: add a check mark reaction to a message ([2ded878](https://github.com/lakinwecker/wokesy/commit/2ded878dfdf41d2ca1c3e9a6f816cf286d329870))
- **wake**: acknowledge a change with a check mark instead of a reply ([af4b6ef](https://github.com/lakinwecker/wokesy/commit/af4b6ef20af11173cb628b7b6fd3eb8e5942a864))

### Documentation

- describe the check mark that answers a change ([698bb15](https://github.com/lakinwecker/wokesy/commit/698bb15f1aa25bfbc13d79b0b93c2841392cdfa4))

### Tooling

- **deploy**: deploy production 0.3.0 ([e475e4b](https://github.com/lakinwecker/wokesy/commit/e475e4be171019bd3a74db11eb7f3bcb12b4e873))

## v0.3.0 — 2026-11-15

### Features

- **wake**: take woke me as wake me ([616e624](https://github.com/lakinwecker/wokesy/commit/616e62464a10daca8a3be1b778b5a853a243cd94))
- **on_call**: sum the roster up in one line and hide the levels in a spoiler ([f9c1e31](https://github.com/lakinwecker/wokesy/commit/f9c1e31de42b18479173dcb28de3abec1b296887))
- **wake**: answer page and wake me in one line ([52116e9](https://github.com/lakinwecker/wokesy/commit/52116e960fcda3e440dd2452d6516cbddc817f25))

### Documentation

- move the detail out of the readme into docs/ ([7f6cfa2](https://github.com/lakinwecker/wokesy/commit/7f6cfa255bf6cf5787b19663f0f041f287fa2f3e))
- write the readme in complete sentences with wokesy and pagerduty as the actors ([3df4e5d](https://github.com/lakinwecker/wokesy/commit/3df4e5db0f3cb5dbb67589eebabe7e4c66482419))
- describe the shorter roster, page, and wake me messages ([d5c0930](https://github.com/lakinwecker/wokesy/commit/d5c093008f971a65efb25e5102d203f2a1269cb9))

### Tooling

- **deploy**: deploy production 0.2.0 ([a63a455](https://github.com/lakinwecker/wokesy/commit/a63a4555b12b087a101700be2dba65c5fb709cd1))

## v0.2.0 — 2026-11-15

### Features

- **pagerduty**: tell the refusal of a shift-based schedule apart from every other ([6fa9e83](https://github.com/lakinwecker/wokesy/commit/6fa9e83096ff0f640cb5fb4a5c644d63e41cb193))
- **pagerduty**: add custom shifts on shift-based schedules ([c554f26](https://github.com/lakinwecker/wokesy/commit/c554f26417d382244f43f95ad44e523c3f21d452))
- **wake**: wake on a shift-based schedule with a custom shift ([dd234ae](https://github.com/lakinwecker/wokesy/commit/dd234ae0e46d840a06354b47df639f9194a51e9b))

### Fixes

- **pagerduty**: say the reason of a refusal once when its errors only repeat the message ([cdf1abf](https://github.com/lakinwecker/wokesy/commit/cdf1abf3b5580ea760e571e2319a9432c66f73c9))
- **pagerduty**: read a shift-based schedule as a target of an escalation rule ([8c5e4d3](https://github.com/lakinwecker/wokesy/commit/8c5e4d345b906a04d6a5f55323852f76d8aa6543))

### Documentation

- describe waking on legacy and on shift-based schedules ([56403c7](https://github.com/lakinwecker/wokesy/commit/56403c7f267d5b0a062f63523d330dc755e557d7))

## v0.1.0 — 2026-11-15

### Features

- **chat,people**: add the chat concepts and the people contracts ([223c885](https://github.com/lakinwecker/wokesy/commit/223c885eb0e5bcc0a77d22809a903f4b306878e6))
- add the settings of zulip, pagerduty, on_call, wake, status, and health ([6f74ae2](https://github.com/lakinwecker/wokesy/commit/6f74ae20dcfd9a2cba2b87bff52e6b4a4bd8024b))
- **app**: load the settings from the environment and wait for shutdown ([819766d](https://github.com/lakinwecker/wokesy/commit/819766d16d29a4d6125cc63d0d5f5faf6efdcc83))
- **zulip**: add the api and the messenger ([c73aafd](https://github.com/lakinwecker/wokesy/commit/c73aafdd877a6b511d0453f4f0fe2844652c0a30))
- **zulip**: read the chat members ([f622745](https://github.com/lakinwecker/wokesy/commit/f6227456f2e57fa95d3e076120e8c96f9e6eaf26))
- **zulip**: keep reading the messages sent to the bot ([ac1eb1d](https://github.com/lakinwecker/wokesy/commit/ac1eb1d80346aa3579c912339a407a8546177f82))
- **pagerduty**: add the on-call concepts as pagerduty defines them ([7c6bfcf](https://github.com/lakinwecker/wokesy/commit/7c6bfcf1940daef830de253c1043f656cb974140))
- **pagerduty**: wrap the rest api that reads who is on call and overrides schedules ([d3f5b71](https://github.com/lakinwecker/wokesy/commit/d3f5b7122e5847060203013ffb5a900a5cb41f72))
- **pagerduty**: page whoever is on call through the events api ([5c84032](https://github.com/lakinwecker/wokesy/commit/5c840327265a0f6709deb9bbe7d744bdc76a9ab3))
- **health**: keep checking pagerduty and zulip and publish what changed ([f4a69f4](https://github.com/lakinwecker/wokesy/commit/f4a69f4c69a949b848b4db12cf01452154f16aa2))
- **status**: post startup, pagerduty's health, and shutdown to a topic per instance ([df95b91](https://github.com/lakinwecker/wokesy/commit/df95b91c4c301b11f4f9569de2b9f0c2395d1e01))
- **app**: answer /livez always and /readyz while pagerduty is available ([1acbd2e](https://github.com/lakinwecker/wokesy/commit/1acbd2e7d2305a90d104d29d9cb7b139e1f014b7))
- **people**: match pagerduty users to chat users and keep the directory fresh ([a85d175](https://github.com/lakinwecker/wokesy/commit/a85d17571b5707bb8a5bd8cdc434cc41c37bddbf))
- **on_call**: keep the roster in sync with pagerduty and announce its changes ([beb0c06](https://github.com/lakinwecker/wokesy/commit/beb0c06a6aa65c298c304c43cd4c17423d1d0ce9))
- **wake**: read the channels that take commands and the page cooldown ([5a5fb87](https://github.com/lakinwecker/wokesy/commit/5a5fb878e9fbf6472420377c10a4b92d45f91c70))
- **wake**: answer page, wake me, stop, back to defaults, who, and help ([4ece4bc](https://github.com/lakinwecker/wokesy/commit/4ece4bce11e1b9458ce46a874e5c54a1b2c6535f))
- **app**: run every job, end the process when one stops, and shut down in order ([41294c7](https://github.com/lakinwecker/wokesy/commit/41294c7400be5c2b4305cbdeedaa74d44ab26931))
- **on_call**: limit the roster to the escalation policies named in the configuration ([bbf5574](https://github.com/lakinwecker/wokesy/commit/bbf557411aff7abee4eb742cd33675ce55548e7b))
- **wake**: wake only on the schedules named in the configuration ([b46bc4f](https://github.com/lakinwecker/wokesy/commit/b46bc4f6d744aa2eae0474c91c7e2f6e84e3d65b))
- **people**: log each new or changed match by its ids and its rule ([00b5f66](https://github.com/lakinwecker/wokesy/commit/00b5f66c84d7558814cfecbd1ceb38ce7a6b9ff6))

### Fixes

- **wake**: say whom level 1 also pages in words that fit one person or many ([8e09249](https://github.com/lakinwecker/wokesy/commit/8e092497cedb934c42f434c3dac3be3098d2d350))
- **zulip**: take a channel message as a command only when it starts with the bot's mention ([3795fab](https://github.com/lakinwecker/wokesy/commit/3795fab6a52a6cec39f89497cf4a8ffc93995f86))
- **wake**: leave a command word in backticks or quotes unrecognized ([48e619a](https://github.com/lakinwecker/wokesy/commit/48e619a03c50259e4fbe5be7ee3f04b21ffab590))
- **wake**: make every mention in a page's text silent before quoting it ([3e4b6c2](https://github.com/lakinwecker/wokesy/commit/3e4b6c209d9bca7cc3fb01a5a0d9d6402a1cff7d))
- **on_call**: show every name from pagerduty with its mentions silenced ([1502277](https://github.com/lakinwecker/wokesy/commit/150227761aee7a618379764d7fe7b0c503b71369))
- **wake**: say in help that back to defaults ends only the overrides wokesy made ([99627b7](https://github.com/lakinwecker/wokesy/commit/99627b75c0e075c136d53dbe96969710e4f862c8))
- **pagerduty**: give up on creating or deleting an override after ten seconds ([096cb6b](https://github.com/lakinwecker/wokesy/commit/096cb6bfd7a14815e965b385f1173833352fd608))
- **wake**: say that an override may exist when pagerduty's answer was lost ([33f1db5](https://github.com/lakinwecker/wokesy/commit/33f1db5b93fb91ff57d880e015dc9a5a251f36ca))
- **people**: do not match by name a member that a link or an email already matched ([ecb18a6](https://github.com/lakinwecker/wokesy/commit/ecb18a67ed2e48dba7bc1315d9be3d76388c6540))
- **pagerduty**: skip a record that cannot be read instead of failing the whole list ([9ce1438](https://github.com/lakinwecker/wokesy/commit/9ce1438239f825c7180b0d7c7cf0917165e85214))
- **zulip**: wait before registering again when a new event queue is rejected at once ([191182d](https://github.com/lakinwecker/wokesy/commit/191182d6304a39504f266cd6a7d9d0a2966d6a26))
- **zulip**: log an event without an id or a type without what it holds ([1728a42](https://github.com/lakinwecker/wokesy/commit/1728a4217be42e24d9d38f8ba73549e3393586fd))
- **app**: start the jobs before the start is posted, and bound the status posts and the http shutdown ([167f8f9](https://github.com/lakinwecker/wokesy/commit/167f8f9a97803f2e2526c8b7cc6f04fa87769b1e))

### Refactoring

- **chat**: remove the mention that notifies ([842d168](https://github.com/lakinwecker/wokesy/commit/842d1689386d32a0f62c7bffadb641920f96ae7f))
- **pagerduty**: return the id of a created override and stop listing overrides ([f0979af](https://github.com/lakinwecker/wokesy/commit/f0979afadf21d4f2d51b65e018663df2325a4380))
- **wake**: read the command channels without an error that cannot happen ([1bf0c09](https://github.com/lakinwecker/wokesy/commit/1bf0c09eff5994ec6186ba9783f1694881be8dec))

### Documentation

- add the readme, the license, and the release and deploy guide ([e386d52](https://github.com/lakinwecker/wokesy/commit/e386d52e2529ac435c1de7df6717441aea448566))
- bring the readme and the agents guide in line with what was built ([1eca542](https://github.com/lakinwecker/wokesy/commit/1eca5420cc378a4b402fb5d230f61fa40953064b))
- describe the wake schedules, the chosen policies, and the rest of this pass ([adcbd6f](https://github.com/lakinwecker/wokesy/commit/adcbd6fc61bd0f0c92faed9643533d4c923b0d93))

### Tests

- **app**: run the whole application against a fake pagerduty and a fake zulip ([6e8d331](https://github.com/lakinwecker/wokesy/commit/6e8d331312821a23b0adbaebaff864473e55fed4))

### Tooling

- set up devenv, secretspec, and the cargo project ([c6ad07f](https://github.com/lakinwecker/wokesy/commit/c6ad07f36fec24a05bdfc7497d18bedff272179f))
- **deploy**: add the kubernetes deployment in kcl ([1b30685](https://github.com/lakinwecker/wokesy/commit/1b306854424a1c3b57d6b087266d5b4cabc8e9a4))
- add the nix flake, the container image, and the release commands ([0a6f0dd](https://github.com/lakinwecker/wokesy/commit/0a6f0dd908ca491c10d8c88efa439d5e9cd83e04))
- add the build, release, and deploy workflows ([d3fc033](https://github.com/lakinwecker/wokesy/commit/d3fc033df469a09d56a06b085969f5abed40ea09))
- **deploy**: keep kubernetes from adding service variables to the pod ([58ed409](https://github.com/lakinwecker/wokesy/commit/58ed4092434f6bb181604ad1eb3333cfd2ae4645))
- move the home to lakinwecker/wokesy ([99b5d6e](https://github.com/lakinwecker/wokesy/commit/99b5d6e8c9f6b232e32dd08e7855da95b9abfd26))

