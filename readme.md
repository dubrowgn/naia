# naia-lite

This is a stripped down version of [naia](https://github.com/naia-lib/naia) for
use specifically with deterministic lockstep applications.

## Major changes/improvements

* Code base reduced from ~39k lines of rust to ~11.8k (as of 2024-10-21)
* Removed bevy, hecs, and miniquad from naia core (no more naia changes necessary
  after new bevy/hecs/miniquad releases)
* Removed entity replication; Entity replication is a huge chunk of the naia
  implementation. Deterministic applications don't benefit from these features.
  (~14.5k LoC)
* Removed tick buffered messages; These tend to be tighly coupled to the
  application, and can be implmented on top of the other networking channels. This
  removes a large amount of special casing in naia, and can be implemented
  at the application level more robustly with a fraction of the code. (~2.5k LoC)
* Removed WASM
* Full support for client and server being in the same process
* Messages are actually recieved in order, which allows deterministic applications
* Split send and receive operations

## Other improvements

* Various bug fixes
* Dramatically improved best case latency
	* Immediate batch send avoids waiting a full tick duration in several places
	* Best case RTT is now ~0-2ms (depending on application), instead of ~100ms
* Reduced wait time between initial heartbeats (removes ~2.5s durting connect)
* Use recycling u16 for UserKey
* Removed a bunch of smaller items:
	* user rooms
	* command history
	* BigMap
	* PingStore
	* and a lot more

## To do (As of 2025-06-25)

* Fully expose server configuration
* Fix data packets drop race condition on connect
