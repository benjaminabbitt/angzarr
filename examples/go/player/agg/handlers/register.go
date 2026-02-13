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

// HandleRegisterPlayer handles the RegisterPlayer command.
func HandleRegisterPlayer(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state PlayerState,
	seq uint32,
) (*pb.EventBook, error) {
	if state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Player already exists")
	}

	var cmd examples.RegisterPlayer
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if cmd.DisplayName == "" {
		return nil, angzarr.NewCommandRejectedError("display_name is required")
	}
	if cmd.Email == "" {
		return nil, angzarr.NewCommandRejectedError("email is required")
	}

	event := &examples.PlayerRegistered{
		DisplayName:  cmd.DisplayName,
		Email:        cmd.Email,
		PlayerType:   cmd.PlayerType,
		AiModelId:    cmd.AiModelId,
		RegisteredAt: timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
