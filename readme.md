# naia-lite

This is a stripped down version of [naia](https://github.com/naia-lib/naia) for
use specifically with deterministic lockstep applications.

## Major Removals

* Code base reduced from ~39k lines of rust to ~7.8k (as of 2025-08-07)
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
* Removed webrtc (~1.3k LoC)
* Combined client/server datapaths and time managers (~600 LoC)

## Major Additions

* Full support for client and server being in the same process
* Messages are actually recieved in order, which allows deterministic applications
* Split send and receive operations
* Added message and packet-level performance counters
* Full chacha20poly1305 encryption with x25519 Diffie–Hellman key exchange

## Other improvements

* Various bug fixes
* Dramatically improved best case latency
	* Immediate batch send avoids waiting a full tick in several places
	* Best case RTT is now ~0-2ms (depending on application), instead of ~100ms
* Removed handshake ping/pongs (removes ~2.5s during connect)
* Reduced handshake to 2 round trips by combining validation and connection requests
* Use recycling u16 for UserKey
* Server notifies clients on shutdown
* Removed a bunch of smaller items:
	* user rooms
	* command history
	* BigMap
	* PingStore
	* SocketConfig
	* and a lot more

## To do

* Fix server drops data packets race condition on connect
* Add encryption (and remove ring)
* Fix or remove fragmentation logic