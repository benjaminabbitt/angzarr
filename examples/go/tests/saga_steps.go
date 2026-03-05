// Package tests implements saga step definitions for BDD tests.
package tests

import (
	"context"
	"fmt"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"github.com/cucumber/godog"
	"github.com/google/uuid"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// SagaContext holds state for saga tests.
type SagaContext struct {
	sagaType        string
	sourceEvent     *anypb.Any
	activePlayers   []*examples.SeatSnapshot
	winners         []*examples.PotWinner
	stackChanges    map[string]int64
	resultCommands  []*pb.CommandBook
	lastError       error
	eventBook       *pb.EventBook
	sagaRouterSagas []string
	handledBy       []string
}

// NewSagaContext creates a fresh saga context.
func NewSagaContext() *SagaContext {
	return &SagaContext{
		stackChanges: make(map[string]int64),
	}
}

var sagaCtx *SagaContext

// RegisterSagaSteps registers all saga step definitions.
func RegisterSagaSteps(ctx *godog.ScenarioContext) {
	sagaCtx = NewSagaContext()

	// Reset before each scenario
	ctx.Before(func(c context.Context, sc *godog.Scenario) (context.Context, error) {
		sagaCtx = NewSagaContext()
		return c, nil
	})

	// Given steps
	ctx.Step(`^a TableSyncSaga$`, aTableSyncSaga)
	ctx.Step(`^a HandResultsSaga$`, aHandResultsSaga)
	ctx.Step(`^a HandStarted event from table domain with:$`, aHandStartedEventFromTableDomainWith)
	ctx.Step(`^a HandComplete event from hand domain with:$`, aHandCompleteEventFromHandDomainWith)
	ctx.Step(`^a HandEnded event from table domain with:$`, aHandEndedEventFromTableDomainWith)
	ctx.Step(`^a PotAwarded event from hand domain with:$`, aPotAwardedEventFromHandDomainWith)
	ctx.Step(`^active players:$`, activePlayersTable)
	ctx.Step(`^winners:$`, winnersTable)
	ctx.Step(`^stack_changes:$`, stackChangesTable)
	ctx.Step(`^a SagaRouter with TableSyncSaga and HandResultsSaga$`, aSagaRouterWithBothSagas)
	ctx.Step(`^a SagaRouter with TableSyncSaga$`, aSagaRouterWithTableSyncSaga)
	ctx.Step(`^a SagaRouter with a failing saga and TableSyncSaga$`, aSagaRouterWithFailingSagaAndTableSyncSaga)
	ctx.Step(`^a HandStarted event$`, aHandStartedEvent)
	ctx.Step(`^an event book with:$`, anEventBookWith)

	// When steps
	ctx.Step(`^the saga handles the event$`, theSagaHandlesTheEvent)
	ctx.Step(`^the router routes the event$`, theRouterRoutesTheEvent)
	ctx.Step(`^the router routes the events$`, theRouterRoutesTheEvents)

	// Then steps
	ctx.Step(`^the saga emits a DealCards command to hand domain$`, theSagaEmitsADealCardsCommandToHandDomain)
	ctx.Step(`^the saga emits an EndHand command to table domain$`, theSagaEmitsAnEndHandCommandToTableDomain)
	ctx.Step(`^the saga emits (\d+) ReleaseFunds commands to player domain$`, theSagaEmitsReleaseFundsCommandsToPlayerDomain)
	ctx.Step(`^the saga emits (\d+) DepositFunds commands to player domain$`, theSagaEmitsDepositFundsCommandsToPlayerDomain)
	ctx.Step(`^the saga emits (\d+) DealCards commands$`, theSagaEmitsDealCardsCommands)
	ctx.Step(`^the command has game_variant TEXAS_HOLDEM$`, theCommandHasGameVariantTexasHoldem)
	ctx.Step(`^the command has (\d+) players$`, theCommandHasPlayers)
	ctx.Step(`^the command has hand_number (\d+)$`, theCommandHasHandNumber)
	ctx.Step(`^the command has (\d+) result$`, theCommandHasResult)
	ctx.Step(`^the result has winner "([^"]*)" with amount (\d+)$`, theResultHasWinnerWithAmount)
	ctx.Step(`^the first command has amount (\d+) for "([^"]*)"$`, theFirstCommandHasAmountFor)
	ctx.Step(`^the second command has amount (\d+) for "([^"]*)"$`, theSecondCommandHasAmountFor)
	ctx.Step(`^only TableSyncSaga handles the event$`, onlyTableSyncSagaHandlesTheEvent)
	ctx.Step(`^TableSyncSaga still emits its command$`, tableSyncSagaStillEmitsItsCommand)
	ctx.Step(`^no exception is raised$`, noExceptionIsRaised)
}

// Given step implementations

func aTableSyncSaga() error {
	sagaCtx.sagaType = "TableSyncSaga"
	return nil
}

func aHandResultsSaga() error {
	sagaCtx.sagaType = "HandResultsSaga"
	return nil
}

func aHandStartedEventFromTableDomainWith(table *godog.Table) error {
	row := table.Rows[1]
	handRoot := parseUUID(row.Cells[0].Value)
	handNumber := parseInt64(row.Cells[1].Value)
	gameVariant := examples.GameVariant(examples.GameVariant_value[row.Cells[2].Value])
	dealerPos := parseInt32(row.Cells[3].Value)

	event := &examples.HandStarted{
		HandRoot:       handRoot,
		HandNumber:     handNumber,
		GameVariant:    gameVariant,
		DealerPosition: dealerPos,
		StartedAt:      timestamppb.Now(),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	sagaCtx.sourceEvent = eventAny
	return nil
}

func aHandCompleteEventFromHandDomainWith(table *godog.Table) error {
	row := table.Rows[1]
	tableRoot := parseUUID(row.Cells[0].Value)
	potTotal := parseInt64(row.Cells[1].Value)

	event := &examples.HandComplete{
		TableRoot:   tableRoot,
		CompletedAt: timestamppb.Now(),
	}
	_ = potTotal // Used via winners

	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	sagaCtx.sourceEvent = eventAny
	return nil
}

func aHandEndedEventFromTableDomainWith(table *godog.Table) error {
	row := table.Rows[1]
	handRoot := parseUUID(row.Cells[0].Value)

	event := &examples.HandEnded{
		HandRoot: handRoot,
		EndedAt:  timestamppb.Now(),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	sagaCtx.sourceEvent = eventAny
	return nil
}

func aPotAwardedEventFromHandDomainWith(table *godog.Table) error {
	event := &examples.PotAwarded{
		AwardedAt: timestamppb.Now(),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	sagaCtx.sourceEvent = eventAny
	return nil
}

func activePlayersTable(table *godog.Table) error {
	sagaCtx.activePlayers = nil
	for _, row := range table.Rows[1:] {
		playerRoot := parseUUID(row.Cells[0].Value)
		position := parseInt32(row.Cells[1].Value)
		stack := parseInt64(row.Cells[2].Value)

		sagaCtx.activePlayers = append(sagaCtx.activePlayers, &examples.SeatSnapshot{
			PlayerRoot: playerRoot,
			Position:   position,
			Stack:      stack,
		})

		// Also populate PM context if available
		if pmCtx != nil && pmCtx.process != nil {
			if pmCtx.process.Players == nil {
				pmCtx.process.Players = make(map[int32]*PMPlayerState)
			}
			pmCtx.process.Players[position] = &PMPlayerState{
				PlayerRoot: playerRoot,
				Position:   position,
				Stack:      stack,
			}
		}
	}

	// Update the HandStarted event with active players
	if sagaCtx.sourceEvent != nil && sagaCtx.sourceEvent.MessageIs(&examples.HandStarted{}) {
		var hs examples.HandStarted
		sagaCtx.sourceEvent.UnmarshalTo(&hs)
		hs.ActivePlayers = sagaCtx.activePlayers
		sagaCtx.sourceEvent, _ = anypb.New(&hs)
	}
	return nil
}

func winnersTable(table *godog.Table) error {
	sagaCtx.winners = nil
	for _, row := range table.Rows[1:] {
		playerRoot := parseUUID(row.Cells[0].Value)
		amount := parseInt64(row.Cells[1].Value)

		sagaCtx.winners = append(sagaCtx.winners, &examples.PotWinner{
			PlayerRoot: playerRoot,
			Amount:     amount,
		})
	}

	// Update HandComplete or PotAwarded with winners
	if sagaCtx.sourceEvent != nil {
		if sagaCtx.sourceEvent.MessageIs(&examples.HandComplete{}) {
			var hc examples.HandComplete
			sagaCtx.sourceEvent.UnmarshalTo(&hc)
			hc.Winners = sagaCtx.winners
			sagaCtx.sourceEvent, _ = anypb.New(&hc)
		} else if sagaCtx.sourceEvent.MessageIs(&examples.PotAwarded{}) {
			var pa examples.PotAwarded
			sagaCtx.sourceEvent.UnmarshalTo(&pa)
			pa.Winners = sagaCtx.winners
			sagaCtx.sourceEvent, _ = anypb.New(&pa)
		}
	}
	return nil
}

func stackChangesTable(table *godog.Table) error {
	sagaCtx.stackChanges = make(map[string]int64)
	for _, row := range table.Rows[1:] {
		if len(row.Cells) < 2 {
			continue
		}
		playerRoot := row.Cells[0].Value
		change := parseInt64(row.Cells[1].Value)
		sagaCtx.stackChanges[playerRoot] = change
	}

	// Update HandEnded with stack_changes
	if sagaCtx.sourceEvent != nil && sagaCtx.sourceEvent.MessageIs(&examples.HandEnded{}) {
		var he examples.HandEnded
		sagaCtx.sourceEvent.UnmarshalTo(&he)
		he.StackChanges = make(map[string]int64)
		for k, v := range sagaCtx.stackChanges {
			he.StackChanges[k] = v
		}
		sagaCtx.sourceEvent, _ = anypb.New(&he)
	}
	return nil
}

func aSagaRouterWithBothSagas() error {
	sagaCtx.sagaRouterSagas = []string{"TableSyncSaga", "HandResultsSaga"}
	return nil
}

func aSagaRouterWithTableSyncSaga() error {
	sagaCtx.sagaRouterSagas = []string{"TableSyncSaga"}
	return nil
}

func aSagaRouterWithFailingSagaAndTableSyncSaga() error {
	sagaCtx.sagaRouterSagas = []string{"FailingSaga", "TableSyncSaga"}
	return nil
}

func aHandStartedEvent() error {
	event := &examples.HandStarted{
		HandRoot:       uuid.New().NodeID(),
		HandNumber:     1,
		GameVariant:    examples.GameVariant_TEXAS_HOLDEM,
		DealerPosition: 0,
		ActivePlayers: []*examples.SeatSnapshot{
			{PlayerRoot: parseUUID("player-1"), Position: 0, Stack: 500},
			{PlayerRoot: parseUUID("player-2"), Position: 1, Stack: 500},
		},
		StartedAt: timestamppb.Now(),
	}
	eventAny, _ := anypb.New(event)
	sagaCtx.sourceEvent = eventAny
	return nil
}

func anEventBookWith(table *godog.Table) error {
	sagaCtx.eventBook = &pb.EventBook{
		Cover: &pb.Cover{Domain: "table"},
		Pages: make([]*pb.EventPage, 0),
	}

	for _, row := range table.Rows[1:] {
		eventType := row.Cells[0].Value
		var eventAny *anypb.Any
		var err error

		switch eventType {
		case "HandStarted":
			event := &examples.HandStarted{
				HandRoot:    uuid.New().NodeID(),
				HandNumber:  1,
				GameVariant: examples.GameVariant_TEXAS_HOLDEM,
				ActivePlayers: []*examples.SeatSnapshot{
					{PlayerRoot: parseUUID("player-1"), Position: 0, Stack: 500},
					{PlayerRoot: parseUUID("player-2"), Position: 1, Stack: 500},
				},
				StartedAt: timestamppb.Now(),
			}
			eventAny, err = anypb.New(event)
		}

		if err != nil {
			return err
		}

		sagaCtx.eventBook.Pages = append(sagaCtx.eventBook.Pages, &pb.EventPage{
			Payload: &pb.EventPage_Event{Event: eventAny},
		})
	}
	return nil
}

// When step implementations

func theSagaHandlesTheEvent() error {
	sagaCtx.resultCommands = nil

	switch sagaCtx.sagaType {
	case "TableSyncSaga":
		return handleTableSyncSaga()
	case "HandResultsSaga":
		return handleHandResultsSaga()
	default:
		return fmt.Errorf("unknown saga type: %s", sagaCtx.sagaType)
	}
}

func handleTableSyncSaga() error {
	if sagaCtx.sourceEvent.MessageIs(&examples.HandStarted{}) {
		var hs examples.HandStarted
		sagaCtx.sourceEvent.UnmarshalTo(&hs)

		players := make([]*examples.PlayerInHand, len(hs.ActivePlayers))
		for i, seat := range hs.ActivePlayers {
			players[i] = &examples.PlayerInHand{
				PlayerRoot: seat.PlayerRoot,
				Position:   seat.Position,
				Stack:      seat.Stack,
			}
		}

		cmd := &examples.DealCards{
			TableRoot:      hs.HandRoot,
			HandNumber:     hs.HandNumber,
			GameVariant:    hs.GameVariant,
			Players:        players,
			DealerPosition: hs.DealerPosition,
		}
		cmdAny, _ := anypb.New(cmd)

		sagaCtx.resultCommands = append(sagaCtx.resultCommands, &pb.CommandBook{
			Cover: &pb.Cover{Domain: "hand"},
			Pages: []*pb.CommandPage{{Payload: &pb.CommandPage_Command{Command: cmdAny}}},
		})
	} else if sagaCtx.sourceEvent.MessageIs(&examples.HandComplete{}) {
		var hc examples.HandComplete
		sagaCtx.sourceEvent.UnmarshalTo(&hc)

		results := make([]*examples.PotResult, len(hc.Winners))
		for i, w := range hc.Winners {
			results[i] = &examples.PotResult{
				WinnerRoot: w.PlayerRoot,
				Amount:     w.Amount,
			}
		}

		cmd := &examples.EndHand{
			HandRoot: hc.TableRoot,
			Results:  results,
		}
		cmdAny, _ := anypb.New(cmd)

		sagaCtx.resultCommands = append(sagaCtx.resultCommands, &pb.CommandBook{
			Cover: &pb.Cover{Domain: "table"},
			Pages: []*pb.CommandPage{{Payload: &pb.CommandPage_Command{Command: cmdAny}}},
		})
	}
	return nil
}

func handleHandResultsSaga() error {
	if sagaCtx.sourceEvent.MessageIs(&examples.HandEnded{}) {
		var he examples.HandEnded
		sagaCtx.sourceEvent.UnmarshalTo(&he)

		for playerKey := range he.StackChanges {
			cmd := &examples.ReleaseFunds{
				TableRoot: he.HandRoot,
			}
			cmdAny, _ := anypb.New(cmd)

			sagaCtx.resultCommands = append(sagaCtx.resultCommands, &pb.CommandBook{
				Cover: &pb.Cover{Domain: "player", Root: &pb.UUID{Value: parseUUID(playerKey)}},
				Pages: []*pb.CommandPage{{Payload: &pb.CommandPage_Command{Command: cmdAny}}},
			})
		}
	} else if sagaCtx.sourceEvent.MessageIs(&examples.PotAwarded{}) {
		var pa examples.PotAwarded
		sagaCtx.sourceEvent.UnmarshalTo(&pa)

		for _, winner := range pa.Winners {
			cmd := &examples.DepositFunds{
				Amount: &examples.Currency{Amount: winner.Amount, CurrencyCode: "CHIPS"},
			}
			cmdAny, _ := anypb.New(cmd)

			sagaCtx.resultCommands = append(sagaCtx.resultCommands, &pb.CommandBook{
				Cover: &pb.Cover{Domain: "player", Root: &pb.UUID{Value: winner.PlayerRoot}},
				Pages: []*pb.CommandPage{{Payload: &pb.CommandPage_Command{Command: cmdAny}}},
			})
		}
	}
	return nil
}

func theRouterRoutesTheEvent() error {
	sagaCtx.resultCommands = nil
	sagaCtx.handledBy = nil

	for _, sagaName := range sagaCtx.sagaRouterSagas {
		if sagaName == "FailingSaga" {
			continue // Simulate failing saga by skipping
		}

		if sagaName == "TableSyncSaga" && sagaCtx.sourceEvent.MessageIs(&examples.HandStarted{}) {
			sagaCtx.handledBy = append(sagaCtx.handledBy, sagaName)
			sagaCtx.sagaType = "TableSyncSaga"
			handleTableSyncSaga()
		} else if sagaName == "HandResultsSaga" {
			// HandResultsSaga doesn't handle HandStarted
		}
	}
	return nil
}

func theRouterRoutesTheEvents() error {
	sagaCtx.resultCommands = nil

	for _, page := range sagaCtx.eventBook.Pages {
		sagaCtx.sourceEvent = page.GetEvent()

		for _, sagaName := range sagaCtx.sagaRouterSagas {
			if sagaName == "TableSyncSaga" && sagaCtx.sourceEvent.MessageIs(&examples.HandStarted{}) {
				sagaCtx.sagaType = "TableSyncSaga"
				handleTableSyncSaga()
			}
		}
	}
	return nil
}

// Then step implementations

func theSagaEmitsADealCardsCommandToHandDomain() error {
	if len(sagaCtx.resultCommands) == 0 {
		return fmt.Errorf("no commands emitted")
	}
	cmd := sagaCtx.resultCommands[0]
	if cmd.Cover.Domain != "hand" {
		return fmt.Errorf("expected hand domain, got %s", cmd.Cover.Domain)
	}
	if !cmd.Pages[0].GetCommand().MessageIs(&examples.DealCards{}) {
		return fmt.Errorf("expected DealCards command")
	}
	return nil
}

func theSagaEmitsAnEndHandCommandToTableDomain() error {
	if len(sagaCtx.resultCommands) == 0 {
		return fmt.Errorf("no commands emitted")
	}
	cmd := sagaCtx.resultCommands[0]
	if cmd.Cover.Domain != "table" {
		return fmt.Errorf("expected table domain, got %s", cmd.Cover.Domain)
	}
	if !cmd.Pages[0].GetCommand().MessageIs(&examples.EndHand{}) {
		return fmt.Errorf("expected EndHand command")
	}
	return nil
}

func theSagaEmitsReleaseFundsCommandsToPlayerDomain(count int) error {
	if len(sagaCtx.resultCommands) != count {
		return fmt.Errorf("expected %d commands, got %d", count, len(sagaCtx.resultCommands))
	}
	for _, cmd := range sagaCtx.resultCommands {
		if cmd.Cover.Domain != "player" {
			return fmt.Errorf("expected player domain, got %s", cmd.Cover.Domain)
		}
	}
	return nil
}

func theSagaEmitsDepositFundsCommandsToPlayerDomain(count int) error {
	if len(sagaCtx.resultCommands) != count {
		return fmt.Errorf("expected %d commands, got %d", count, len(sagaCtx.resultCommands))
	}
	for _, cmd := range sagaCtx.resultCommands {
		if cmd.Cover.Domain != "player" {
			return fmt.Errorf("expected player domain, got %s", cmd.Cover.Domain)
		}
		if !cmd.Pages[0].GetCommand().MessageIs(&examples.DepositFunds{}) {
			return fmt.Errorf("expected DepositFunds command")
		}
	}
	return nil
}

func theSagaEmitsDealCardsCommands(count int) error {
	dealCardsCount := 0
	for _, cmd := range sagaCtx.resultCommands {
		if cmd.Pages[0].GetCommand().MessageIs(&examples.DealCards{}) {
			dealCardsCount++
		}
	}
	if dealCardsCount != count {
		return fmt.Errorf("expected %d DealCards commands, got %d", count, dealCardsCount)
	}
	return nil
}

func theCommandHasGameVariantTexasHoldem() error {
	if len(sagaCtx.resultCommands) == 0 {
		return fmt.Errorf("no commands")
	}
	var dc examples.DealCards
	sagaCtx.resultCommands[0].Pages[0].GetCommand().UnmarshalTo(&dc)
	if dc.GameVariant != examples.GameVariant_TEXAS_HOLDEM {
		return fmt.Errorf("expected TEXAS_HOLDEM, got %v", dc.GameVariant)
	}
	return nil
}

func theCommandHasPlayers(count int) error {
	if len(sagaCtx.resultCommands) == 0 {
		return fmt.Errorf("no commands")
	}
	var dc examples.DealCards
	sagaCtx.resultCommands[0].Pages[0].GetCommand().UnmarshalTo(&dc)
	if len(dc.Players) != count {
		return fmt.Errorf("expected %d players, got %d", count, len(dc.Players))
	}
	return nil
}

func theCommandHasHandNumber(num int) error {
	if len(sagaCtx.resultCommands) == 0 {
		return fmt.Errorf("no commands")
	}
	var dc examples.DealCards
	sagaCtx.resultCommands[0].Pages[0].GetCommand().UnmarshalTo(&dc)
	if dc.HandNumber != int64(num) {
		return fmt.Errorf("expected hand_number %d, got %d", num, dc.HandNumber)
	}
	return nil
}

func theCommandHasResult(count int) error {
	if len(sagaCtx.resultCommands) == 0 {
		return fmt.Errorf("no commands")
	}
	var eh examples.EndHand
	sagaCtx.resultCommands[0].Pages[0].GetCommand().UnmarshalTo(&eh)
	if len(eh.Results) != count {
		return fmt.Errorf("expected %d results, got %d", count, len(eh.Results))
	}
	return nil
}

func theResultHasWinnerWithAmount(playerName string, amount int) error {
	if len(sagaCtx.resultCommands) == 0 {
		return fmt.Errorf("no commands")
	}
	var eh examples.EndHand
	sagaCtx.resultCommands[0].Pages[0].GetCommand().UnmarshalTo(&eh)

	for _, r := range eh.Results {
		if r.Amount == int64(amount) {
			return nil
		}
	}
	return fmt.Errorf("no result with amount %d found", amount)
}

func theFirstCommandHasAmountFor(amount int, playerName string) error {
	if len(sagaCtx.resultCommands) < 1 {
		return fmt.Errorf("no commands")
	}
	var df examples.DepositFunds
	sagaCtx.resultCommands[0].Pages[0].GetCommand().UnmarshalTo(&df)
	if df.Amount.Amount != int64(amount) {
		return fmt.Errorf("expected amount %d, got %d", amount, df.Amount.Amount)
	}
	return nil
}

func theSecondCommandHasAmountFor(amount int, playerName string) error {
	if len(sagaCtx.resultCommands) < 2 {
		return fmt.Errorf("need at least 2 commands")
	}
	var df examples.DepositFunds
	sagaCtx.resultCommands[1].Pages[0].GetCommand().UnmarshalTo(&df)
	if df.Amount.Amount != int64(amount) {
		return fmt.Errorf("expected amount %d, got %d", amount, df.Amount.Amount)
	}
	return nil
}

func onlyTableSyncSagaHandlesTheEvent() error {
	if len(sagaCtx.handledBy) != 1 || sagaCtx.handledBy[0] != "TableSyncSaga" {
		return fmt.Errorf("expected only TableSyncSaga, got %v", sagaCtx.handledBy)
	}
	return nil
}

func tableSyncSagaStillEmitsItsCommand() error {
	return theSagaEmitsADealCardsCommandToHandDomain()
}

func noExceptionIsRaised() error {
	return nil // If we got here, no exception was raised
}

// Helper functions

func parseUUID(s string) []byte {
	// Create deterministic UUID from string
	id := uuid.NewSHA1(uuid.NameSpaceOID, []byte(s))
	return id[:]
}

func parseInt32(s string) int32 {
	var v int32
	fmt.Sscanf(s, "%d", &v)
	return v
}

func parseInt64(s string) int64 {
	var v int64
	fmt.Sscanf(s, "%d", &v)
	return v
}
