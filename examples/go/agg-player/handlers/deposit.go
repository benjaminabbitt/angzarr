package handlers

import (
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleDepositFunds handles the DepositFunds command.
func HandleDepositFunds(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state PlayerState,
	seq uint32,
) (*pb.EventBook, error) {
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Player does not exist")
	}

	var cmd examples.DepositFunds
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	amount := int64(0)
	if cmd.Amount != nil {
		amount = cmd.Amount.Amount
	}
	if amount <= 0 {
		return nil, angzarr.NewCommandRejectedError("amount must be positive")
	}

	newBalance := state.Bankroll + amount

	event := &examples.FundsDeposited{
		Amount:      cmd.Amount,
		NewBalance:  &examples.Currency{Amount: newBalance, CurrencyCode: "CHIPS"},
		DepositedAt: timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}
	eventAny.TypeUrl = "type.poker/examples.FundsDeposited"

	return &pb.EventBook{
		Cover: commandBook.Cover,
		Pages: []*pb.EventPage{
			{
				Sequence:  &pb.EventPage_Num{Num: seq},
				Event:     eventAny,
				CreatedAt: timestamppb.New(time.Now()),
			},
		},
	}, nil
}
