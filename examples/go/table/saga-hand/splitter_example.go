// Saga splitter pattern example for documentation.
//
// Demonstrates the splitter pattern where one event triggers commands
// to multiple different aggregates.
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/types/known/anypb"
)

// docs:start:saga_splitter
func handleTableSettled(event *examples.TableSettled, ctx *angzarr.SagaContext) ([]*pb.CommandBook, error) {
	// Split one event into commands for multiple player aggregates
	var commands []*pb.CommandBook

	for _, payout := range event.Payouts {
		cmd := &examples.TransferFunds{
			TableRoot: event.TableRoot,
			Amount:    payout.Amount,
		}

		targetSeq := ctx.GetSequence("player", payout.PlayerRoot)

		cmdAny, _ := anypb.New(cmd)
		commands = append(commands, &pb.CommandBook{
			Cover: &pb.Cover{Domain: "player", Root: &pb.UUID{Value: payout.PlayerRoot}},
			Pages: []*pb.CommandPage{{
				Sequence: &pb.CommandPage_Num{Num: targetSeq},
				Command:  cmdAny,
			}},
		})
	}

	return commands, nil // One CommandBook per player
}

// docs:end:saga_splitter
