# Announcements

This page has what wokesy posts without being asked: who is on call, and its own status. For the replies to commands, see [Commands](commands.md).

Wherever wokesy names a person, it uses a silent mention, so [nobody is pinged](zulip.md#nobody-is-pinged).

## On-call announcements

Wokesy reads who is on call from PagerDuty every minute, and again at once after a command that changes it. It posts in the on-call topic, which [Zulip](zulip.md#the-channels) says how to choose.

When it starts, it posts the whole roster once. The first line names the people whom PagerDuty would alert first, each with the time left on their shift. Under that line is a block named `schedule`. Zulip shows the block closed, and a reader clicks on it to open it. It has every level of each escalation policy that a service uses, with the time each shift ends. With the block open, Zulip shows the roster like this:

```
On call now: Alice for 4 hours

schedule
Infrastructure · Level 1
- nobody

Infrastructure · Level 2
- Alice until 20:00

Infrastructure · Level 3
- Bob until 09:00
- Carol until 09:00
```

After that, it posts only when the set of people on call at some level changes. The first line names the people whom PagerDuty would alert first now, also when the level that changed is another one. The block is named `changes`. It shows only the levels that changed, and the last line of each level says who was on call there before:

```
On call changed: Alice for 30 minutes

changes
Infrastructure · Level 1
- Alice until 16:30
- was: nobody
```

This post is also how wokesy confirms `wake me`, `stop waking me`, and `back to defaults`. Wokesy answers those commands with [a check mark](commands.md#a-check-mark-instead-of-a-reply) on the sender's message and no reply, so the sender finds the result here, in the on-call topic. When the command did not change the set of people on call, wokesy posts nothing here, and the check mark is the only sign.

### The first line

- **Who is named.** PagerDuty skips a level that has nobody on call. So for each escalation policy, wokesy names the people at the first level that has anybody. In the roster above, that is level 2. Wokesy names a person once, even when several policies would alert them first.
- **The time left.** Wokesy gives the time that was left on the shift when it wrote the message, as one unit: `45 minutes`, `4 hours`, `2 days`, `2 weeks`, `3 months`, or `1 year`. It adds 90 seconds and then rounds down, because wokesy reads who is on call once a minute and may write the message a little after the shift began: with 2 hours 59 minutes left it says `3 hours`. It never says less than `1 minute`. A shift without an end, such as that of a person whom the escalation policy names directly, has no time after the name.
- **Nobody at all.** When nobody is on call at any level of any policy that wokesy shows, the first line is `On call now: nobody`, with a warning sign before `nobody`. `On call changed: nobody` has the same sign.

### The block

- A level with nobody on call says `nobody`. With the [recommended escalation policy](pagerduty.md#the-escalation-policy), level 1 says that whenever nobody has asked to be woken.
- The end of a shift is shown to each reader in their own time zone.
- In the `changes` block, the line that starts with `was:` names the people without the ends of their shifts. It says `was: nobody` for a level that was empty.
- In the `changes` block, a person whom `wake me` just put on call ends with `from` and a link to the message that asked, such as `- Alice until 16:30, from #sysadmin > incident`. Wokesy links only a person who was not on call at that level before, at a level of a schedule on which it made the override or custom shift, while it still has that on record. After a restart, or once the person's time is up or `stop waking me` ended it, there is no link.

### What appears, and when

- A shift that only gets a new end time is not a change.
- Only an escalation policy that at least one PagerDuty service uses appears. If `WOKESY_ON_CALL__ESCALATION_POLICIES` is set, only the policies named there appear. If none is left, the roster has no block, and it says `No escalation policy is used by a service, so nobody is on call.`
- After a restart, wokesy posts the whole roster again, because it cannot know what changed while it was down.

While wokesy cannot read PagerDuty, it keeps the roster it has and posts nothing. If Zulip does not take an announcement, wokesy tries again after 30 seconds.

To read PagerDuty more or less often, set `WOKESY_ON_CALL__REFRESH_INTERVAL`.

## Pages

Every [`page`](commands.md#page) is repeated in the on-call topic.

## Status posts

If `WOKESY_STATUS__CHANNEL` is set, wokesy posts in the topic `wokesy status` of that channel:

- when it starts, with its version, an id of the run, and a link to the changelog;
- when PagerDuty works for the first time, when wokesy cannot use PagerDuty, and when PagerDuty works again;
- when it shuts down, with the reason and how long it ran.

A post about PagerDuty says whether wokesy will keep trying or whether a person must fix it, such as a revoked key. Wokesy posts when that changes, not at every check.

Every run of wokesy posts in that one topic, so the topic holds the history of all runs. The topic cannot be set. Wokesy gives each run a new id, and the start post and the log name it. A quiet channel for bots suits these posts best.

Without that channel, these messages are only logged.
