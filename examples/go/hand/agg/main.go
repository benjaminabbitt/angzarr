// Hand bounded context gRPC server using OO pattern.
//
// This aggregate uses the OO-style pattern with embedded AggregateBase,
// method-based handlers, and fluent registration. This contrasts with
// the player aggregate which uses the functional CommandRouter pattern.
package main

import angzarr "github.com/benjaminabbitt/angzarr/client/go"

func main() {
	angzarr.RunOOAggregateServer[HandState, *Hand]("hand", "50203", NewHand)
}
