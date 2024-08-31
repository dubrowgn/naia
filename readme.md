# naia-lite

This is a stripped down version of [naia](https://github.com/naia-lib/naia) for
use specifically with deterministic lockstep applications.

## Major changes/improvements

* Code base reduced from ~39k lines of rust to ~16k (as of 2024-08-31)
* Removed bevy, hecs, and miniquad from naia core (no more naia changes necessary
  after new bevy/hecs/miniquad releases)
* Removed entity replication; Entity replication is a huge chunk of the naia
  implementation. Deterministic applications don't benefit from these features.
  (removed ~14,500 LoC)
* Full support for client and server being in the same process.

## Other improvements

* Removed user rooms

## Why fork naia?

As of this writing, the upstream naia repo hasn't merged even trivial pull
requests in over a year. That combined with the fact that some of the features
and improvements require changes to the naia API means we need to fork it.

## To do (As of 2024-08-31)

* Split send, receive, and tick generation
* Improve latecy
    * Immediately send packets instead of waiting for next tick (reduce RTT by
      &gt;=2 ticks)
    * Tighten send/receive timing estimates
    * Reduce wait time between initial heartbeats (removes ~2.5s durting
      connect)
* Various bug fixes
    * Division by zero during connection when RTT ~0ms
    * Fully expose server configuration
    * Don't discard messages for the current tick (client)
    * Actually apply link conditioning
* Replace BigMap with simple incrementing Id
	* u32 instead of u64?
	* handle wrap around
