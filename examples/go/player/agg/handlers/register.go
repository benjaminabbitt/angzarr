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

func guardRegisterPlayer(state PlayerState) error {
	if state.Exists() {
		return angzarr.NewCommandRejectedError("Player already exists")
	}
	return nil
}

func validateRegisterPlayer(cmd *examples.RegisterPlayer) error {
	if cmd.DisplayName == "" {
		return angzarr.NewCommandRejectedError("display_name is required")
	}
	if cmd.Email == "" {
		return angzarr.NewCommandRejectedError("email is required")
	}
	return nil
}

func computePlayerRegistered(cmd *examples.RegisterPlayer) *examples.PlayerRegistered {
	return &examples.PlayerRegistered{
		DisplayName:  cmd.DisplayName,
		Email:        cmd.Email,
		PlayerType:   cmd.PlayerType,
		AiModelId:    cmd.AiModelId,
		RegisteredAt: timestamppb.New(time.Now()),
	}
}

// HandleRegisterPlayer handles the RegisterPlayer command.
func HandleRegisterPlayer(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state PlayerState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.RegisterPlayer
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardRegisterPlayer(state); err != nil {
		return nil, err
	}
	if err := validateRegisterPlayer(&cmd); err != nil {
		return nil, err
	}

	event := computePlayerRegistered(&cmd)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
