// Table bounded context gRPC server using OO pattern.
//
// This command handler uses the OO-style pattern with embedded CommandHandlerBase,
// method-based handlers, and fluent registration. This contrasts with
// the player command handler which uses the functional CommandRouter pattern.
package main

import angzarr "github.com/benjaminabbitt/angzarr/client/go"

func main() {
	angzarr.RunOOCommandHandlerServer[TableState, *Table]("table", "50202", NewTable)
}
