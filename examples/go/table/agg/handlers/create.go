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

// HandleCreateTable handles the CreateTable command.
func HandleCreateTable(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state TableState,
	seq uint32,
) (*pb.EventBook, error) {
	if state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Table already exists")
	}

	var cmd examples.CreateTable
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if cmd.TableName == "" {
		return nil, angzarr.NewCommandRejectedError("table_name is required")
	}
	if cmd.SmallBlind <= 0 {
		return nil, angzarr.NewCommandRejectedError("small_blind must be positive")
	}
	if cmd.BigBlind <= 0 || cmd.BigBlind < cmd.SmallBlind {
		return nil, angzarr.NewCommandRejectedError("big_blind must be >= small_blind")
	}
	if cmd.MinBuyIn <= 0 {
		return nil, angzarr.NewCommandRejectedError("min_buy_in must be positive")
	}
	if cmd.MaxBuyIn < cmd.MinBuyIn {
		return nil, angzarr.NewCommandRejectedError("max_buy_in must be >= min_buy_in")
	}
	if cmd.MaxPlayers < 2 || cmd.MaxPlayers > 10 {
		return nil, angzarr.NewCommandRejectedError("max_players must be 2-10")
	}

	event := &examples.TableCreated{
		TableName:            cmd.TableName,
		GameVariant:          cmd.GameVariant,
		SmallBlind:           cmd.SmallBlind,
		BigBlind:             cmd.BigBlind,
		MinBuyIn:             cmd.MinBuyIn,
		MaxBuyIn:             cmd.MaxBuyIn,
		MaxPlayers:           cmd.MaxPlayers,
		ActionTimeoutSeconds: cmd.ActionTimeoutSeconds,
		CreatedAt:            timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
