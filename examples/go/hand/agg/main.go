// Hand bounded context gRPC server using OO pattern.
//
// This command handler uses the OO-style pattern with embedded CommandHandlerBase,
// method-based handlers, and fluent registration. This contrasts with
// the player command handler which uses the functional CommandRouter pattern.
package main

import angzarr "github.com/benjaminabbitt/angzarr/client/go"

func main() {
	angzarr.RunOOCommandHandlerServer[HandState, *Hand]("hand", "50203", NewHand)
}
