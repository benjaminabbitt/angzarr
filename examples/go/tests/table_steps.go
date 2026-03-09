package tests

import (
	"context"
	"encoding/hex"
	"fmt"
	"strconv"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"github.com/benjaminabbitt/angzarr/examples/go/table/agg/handlers"
	"github.com/cucumber/godog"
	"github.com/google/uuid"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// TableContext holds state for table aggregate scenarios
type TableContext struct {
	eventPages   []*pb.EventPage
	state        handlers.TableState
	resultEvent  *anypb.Any
	resultEvents []*anypb.Any
	lastError    error
	playerRoots  map[string][]byte // name -> root bytes
}

func newTableContext() *TableContext {
	return &TableContext{
		eventPages:  []*pb.EventPage{},
		state:       handlers.NewTableState(),
		playerRoots: make(map[string][]byte),
	}
}

// InitTableSteps registers table aggregate step definitions
func InitTableSteps(ctx *godog.ScenarioContext) {
	tc := newTableContext()

	// Reset before each scenario
	ctx.Before(func(ctx context.Context, sc *godog.Scenario) (context.Context, error) {
		tc.eventPages = []*pb.EventPage{}
		tc.state = handlers.NewTableState()
		tc.resultEvent = nil
		tc.resultEvents = nil
		tc.lastError = nil
		tc.playerRoots = make(map[string][]byte)
		return ctx, nil
	})

	// Given steps
	ctx.Step(`^no prior events for the table aggregate$`, tc.noPriorEvents)
	ctx.Step(`^a TableCreated event for "([^"]*)"$`, tc.tableCreatedFor)
	ctx.Step(`^a TableCreated event for "([^"]*)" with min_buy_in (\d+)$`, tc.tableCreatedWithMinBuyIn)
	ctx.Step(`^a TableCreated event for "([^"]*)" with max_players (\d+)$`, tc.tableCreatedWithMaxPlayers)
	ctx.Step(`^a PlayerJoined event for player "([^"]*)" at seat (\d+)$`, tc.playerJoinedAtSeat)
	ctx.Step(`^a PlayerJoined event for player "([^"]*)" at seat (\d+) with stack (\d+)$`, tc.playerJoinedAtSeatWithStack)
	ctx.Step(`^a HandStarted event for hand (\d+)$`, tc.handStartedForHand)
	ctx.Step(`^a HandStarted event for hand (\d+) with dealer at seat (\d+)$`, tc.handStartedWithDealer)
	ctx.Step(`^a HandEnded event for hand (\d+)$`, tc.handEndedForHand)

	// When steps
	ctx.Step(`^I handle a CreateTable command with name "([^"]*)" and variant "([^"]*)":$`, tc.handleCreateTableWithVariant)
	ctx.Step(`^I handle a JoinTable command for player "([^"]*)" at seat (-?\d+) with buy-in (\d+)$`, tc.handleJoinTable)
	ctx.Step(`^I handle a LeaveTable command for player "([^"]*)"$`, tc.handleLeaveTable)
	ctx.Step(`^I handle a StartHand command$`, tc.handleStartHand)
	ctx.Step(`^I handle an EndHand command with winner "([^"]*)" winning (\d+)$`, tc.handleEndHandWithWinner)
	ctx.Step(`^I handle an EndHand command with results:$`, tc.handleEndHandWithResults)
	ctx.Step(`^I rebuild the table state$`, tc.rebuildTableState)

	// Then steps
	ctx.Step(`^the result is a (?:examples\.)?TableCreated event$`, tc.resultIsTableCreated)
	ctx.Step(`^the result is a (?:examples\.)?PlayerJoined event$`, tc.resultIsPlayerJoined)
	ctx.Step(`^the result is a (?:examples\.)?PlayerLeft event$`, tc.resultIsPlayerLeft)
	ctx.Step(`^the result is a (?:examples\.)?HandStarted event$`, tc.resultIsHandStarted)
	ctx.Step(`^the result is a (?:examples\.)?HandEnded event$`, tc.resultIsHandEnded)
	ctx.Step(`^the table event has table_name "([^"]*)"$`, tc.eventHasTableName)
	ctx.Step(`^the table event has game_variant "([^"]*)"$`, tc.eventHasGameVariant)
	ctx.Step(`^the table event has small_blind (\d+)$`, tc.eventHasSmallBlind)
	ctx.Step(`^the table event has big_blind (\d+)$`, tc.eventHasBigBlind)
	ctx.Step(`^the table event has seat_position (\d+)$`, tc.eventHasSeatPosition)
	ctx.Step(`^the table event has buy_in_amount (\d+)$`, tc.eventHasBuyInAmount)
	ctx.Step(`^the table event has chips_cashed_out (\d+)$`, tc.eventHasChipsCashedOut)
	ctx.Step(`^the table event has hand_number (\d+)$`, tc.eventHasHandNumber)
	ctx.Step(`^the table event has dealer_position (\d+)$`, tc.eventHasDealerPosition)
	ctx.Step(`^the table event has (\d+) active_players$`, tc.eventHasActivePlayers)
	ctx.Step(`^player "([^"]*)" stack change is (-?\d+)$`, tc.playerStackChangeIs)
	ctx.Step(`^the table state has (\d+) players$`, tc.stateHasPlayers)
	ctx.Step(`^the table state has seat (\d+) occupied by "([^"]*)"$`, tc.stateHasSeatOccupiedBy)
	ctx.Step(`^the table state has status "([^"]*)"$`, tc.stateHasStatus)
	ctx.Step(`^the table state has hand_count (\d+)$`, tc.stateHasHandCount)
	ctx.Step(`^the command fails with "([^"]*)"$`, tc.commandFailsWith)
	// Note: "command fails with status" is registered in common_steps.go
}

// Helper functions

func (tc *TableContext) makeEventPage(event *anypb.Any) *pb.EventPage {
	return &pb.EventPage{
		Header:    &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: uint32(len(tc.eventPages))}},
		CreatedAt: timestamppb.Now(),
		Payload:   &pb.EventPage_Event{Event: event},
	}
}

func (tc *TableContext) addEvent(event *anypb.Any) {
	tc.eventPages = append(tc.eventPages, tc.makeEventPage(event))
	tc.rebuildState()
}

func (tc *TableContext) rebuildState() {
	id := uuid.New()
	eventBook := &pb.EventBook{
		Cover: &pb.Cover{
			Domain: "table",
			Root:   &pb.UUID{Value: id[:]},
		},
		Pages:        tc.eventPages,
		NextSequence: uint32(len(tc.eventPages)),
	}
	tc.state = handlers.RebuildState(eventBook)
}

func (tc *TableContext) makeEventBook() *pb.EventBook {
	id := uuid.New()
	return &pb.EventBook{
		Cover: &pb.Cover{
			Domain: "table",
			Root:   &pb.UUID{Value: id[:]},
		},
		Pages:        tc.eventPages,
		NextSequence: uint32(len(tc.eventPages)),
	}
}

func (tc *TableContext) getOrCreatePlayerRoot(name string) []byte {
	if root, ok := tc.playerRoots[name]; ok {
		return root
	}
	// Create a deterministic root based on name
	root := make([]byte, 16)
	copy(root, []byte(name))
	tc.playerRoots[name] = root
	return root
}

// Given step implementations

func (tc *TableContext) noPriorEvents() error {
	tc.eventPages = []*pb.EventPage{}
	tc.state = handlers.NewTableState()
	return nil
}

func (tc *TableContext) tableCreatedFor(tableName string) error {
	event := &examples.TableCreated{
		TableName:            tableName,
		GameVariant:          examples.GameVariant_TEXAS_HOLDEM,
		SmallBlind:           10,
		BigBlind:             20,
		MinBuyIn:             200,
		MaxBuyIn:             2000,
		MaxPlayers:           9,
		ActionTimeoutSeconds: 30,
		CreatedAt:            timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	tc.addEvent(eventAny)
	return nil
}

func (tc *TableContext) tableCreatedWithMinBuyIn(tableName string, minBuyIn int) error {
	event := &examples.TableCreated{
		TableName:            tableName,
		GameVariant:          examples.GameVariant_TEXAS_HOLDEM,
		SmallBlind:           10,
		BigBlind:             20,
		MinBuyIn:             int64(minBuyIn),
		MaxBuyIn:             int64(minBuyIn) * 10,
		MaxPlayers:           9,
		ActionTimeoutSeconds: 30,
		CreatedAt:            timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	tc.addEvent(eventAny)
	return nil
}

func (tc *TableContext) tableCreatedWithMaxPlayers(tableName string, maxPlayers int) error {
	event := &examples.TableCreated{
		TableName:            tableName,
		GameVariant:          examples.GameVariant_TEXAS_HOLDEM,
		SmallBlind:           10,
		BigBlind:             20,
		MinBuyIn:             200,
		MaxBuyIn:             2000,
		MaxPlayers:           int32(maxPlayers),
		ActionTimeoutSeconds: 30,
		CreatedAt:            timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	tc.addEvent(eventAny)
	return nil
}

func (tc *TableContext) playerJoinedAtSeat(playerName string, seat int) error {
	return tc.playerJoinedAtSeatWithStack(playerName, seat, 1000)
}

func (tc *TableContext) playerJoinedAtSeatWithStack(playerName string, seat, stack int) error {
	playerRoot := tc.getOrCreatePlayerRoot(playerName)
	event := &examples.PlayerJoined{
		PlayerRoot:   playerRoot,
		SeatPosition: int32(seat),
		BuyInAmount:  int64(stack),
		Stack:        int64(stack),
		JoinedAt:     timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	tc.addEvent(eventAny)
	return nil
}

func (tc *TableContext) handStartedForHand(handNumber int) error {
	return tc.handStartedWithDealer(handNumber, 0)
}

func (tc *TableContext) handStartedWithDealer(handNumber, dealerPosition int) error {
	// Generate hand root
	handRoot := make([]byte, 16)
	copy(handRoot, []byte(fmt.Sprintf("hand_%d", handNumber)))

	// Build active players from current state
	var activePlayers []*examples.SeatSnapshot
	for pos, seat := range tc.state.Seats {
		activePlayers = append(activePlayers, &examples.SeatSnapshot{
			Position:   pos,
			PlayerRoot: seat.PlayerRoot,
			Stack:      seat.Stack,
		})
	}

	event := &examples.HandStarted{
		HandRoot:           handRoot,
		HandNumber:         int64(handNumber),
		DealerPosition:     int32(dealerPosition),
		SmallBlindPosition: int32((dealerPosition + 1) % len(tc.state.Seats)),
		BigBlindPosition:   int32((dealerPosition + 2) % len(tc.state.Seats)),
		GameVariant:        tc.state.GameVariant,
		SmallBlind:         tc.state.SmallBlind,
		BigBlind:           tc.state.BigBlind,
		ActivePlayers:      activePlayers,
		StartedAt:          timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	tc.addEvent(eventAny)
	return nil
}

func (tc *TableContext) handEndedForHand(handNumber int) error {
	handRoot := make([]byte, 16)
	copy(handRoot, []byte(fmt.Sprintf("hand_%d", handNumber)))

	event := &examples.HandEnded{
		HandRoot:     handRoot,
		StackChanges: make(map[string]int64),
		Results:      []*examples.PotResult{},
		EndedAt:      timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	tc.addEvent(eventAny)
	return nil
}

// When step implementations

func (tc *TableContext) handleCreateTableWithVariant(tableName, variant string, table *godog.Table) error {
	// Parse table parameters from the Gherkin table
	// Table format: | small_blind | big_blind | min_buy_in | max_buy_in | max_players |
	//               | 5           | 10        | 200        | 1000       | 9           |
	var smallBlind, bigBlind, minBuyIn, maxBuyIn int64 = 10, 20, 200, 2000
	var maxPlayers int32 = 9
	var actionTimeout int32 = 30

	// Header row contains column names
	header := table.Rows[0]
	// Data row contains values
	if len(table.Rows) > 1 {
		data := table.Rows[1]
		for i, cell := range header.Cells {
			if i >= len(data.Cells) {
				break
			}
			value := data.Cells[i].Value
			switch cell.Value {
			case "small_blind":
				v, _ := strconv.ParseInt(value, 10, 64)
				smallBlind = v
			case "big_blind":
				v, _ := strconv.ParseInt(value, 10, 64)
				bigBlind = v
			case "min_buy_in":
				v, _ := strconv.ParseInt(value, 10, 64)
				minBuyIn = v
			case "max_buy_in":
				v, _ := strconv.ParseInt(value, 10, 64)
				maxBuyIn = v
			case "max_players":
				v, _ := strconv.ParseInt(value, 10, 32)
				maxPlayers = int32(v)
			case "action_timeout":
				v, _ := strconv.ParseInt(value, 10, 32)
				actionTimeout = int32(v)
			}
		}
	}

	// Parse game variant
	gameVariant := examples.GameVariant_TEXAS_HOLDEM
	switch strings.ToUpper(variant) {
	case "TEXAS_HOLDEM":
		gameVariant = examples.GameVariant_TEXAS_HOLDEM
	case "OMAHA":
		gameVariant = examples.GameVariant_OMAHA
	case "FIVE_CARD_DRAW":
		gameVariant = examples.GameVariant_FIVE_CARD_DRAW
	}

	cmd := &examples.CreateTable{
		TableName:            tableName,
		GameVariant:          gameVariant,
		SmallBlind:           smallBlind,
		BigBlind:             bigBlind,
		MinBuyIn:             minBuyIn,
		MaxBuyIn:             maxBuyIn,
		MaxPlayers:           maxPlayers,
		ActionTimeoutSeconds: actionTimeout,
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	return tc.dispatchCommand(cmdAny)
}

func (tc *TableContext) handleJoinTable(playerName string, seat, buyIn int) error {
	playerRoot := tc.getOrCreatePlayerRoot(playerName)

	cmd := &examples.JoinTable{
		PlayerRoot:    playerRoot,
		BuyInAmount:   int64(buyIn),
		PreferredSeat: int32(seat),
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	return tc.dispatchCommand(cmdAny)
}

func (tc *TableContext) handleLeaveTable(playerName string) error {
	playerRoot := tc.getOrCreatePlayerRoot(playerName)

	cmd := &examples.LeaveTable{
		PlayerRoot: playerRoot,
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	return tc.dispatchCommand(cmdAny)
}

func (tc *TableContext) handleStartHand() error {
	cmd := &examples.StartHand{}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	return tc.dispatchCommand(cmdAny)
}

func (tc *TableContext) handleEndHandWithWinner(winnerName string, amount int) error {
	winnerRoot := tc.getOrCreatePlayerRoot(winnerName)

	cmd := &examples.EndHand{
		HandRoot: tc.state.CurrentHandRoot,
		Results: []*examples.PotResult{
			{
				WinnerRoot: winnerRoot,
				Amount:     int64(amount),
				PotType:    "main",
			},
		},
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	return tc.dispatchCommand(cmdAny)
}

func (tc *TableContext) handleEndHandWithResults(table *godog.Table) error {
	var results []*examples.PotResult

	for _, row := range table.Rows[1:] { // Skip header
		playerName := row.Cells[0].Value
		amountStr := row.Cells[1].Value
		amount, _ := strconv.ParseInt(amountStr, 10, 64)

		playerRoot := tc.getOrCreatePlayerRoot(playerName)
		results = append(results, &examples.PotResult{
			WinnerRoot: playerRoot,
			Amount:     amount,
			PotType:    "main",
		})
	}

	cmd := &examples.EndHand{
		HandRoot: tc.state.CurrentHandRoot,
		Results:  results,
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	return tc.dispatchCommand(cmdAny)
}

func (tc *TableContext) rebuildTableState() error {
	tc.rebuildState()
	return nil
}

// dispatchCommand dispatches the command to the appropriate handler and captures results
func (tc *TableContext) dispatchCommand(cmdAny *anypb.Any) error {
	eventBook := tc.makeEventBook()

	tc.lastError = nil
	tc.resultEvent = nil
	tc.resultEvents = nil

	// Dispatch to handler functions from handlers package
	switch {
	case cmdAny.MessageIs(&examples.CreateTable{}):
		result, err := handlers.HandleCreateTable(eventBook, cmdAny, tc.state)
		tc.lastError = err
		if err == nil && result != nil {
			tc.resultEvent = result
		}
	case cmdAny.MessageIs(&examples.JoinTable{}):
		result, err := handlers.HandleJoinTable(eventBook, cmdAny, tc.state)
		tc.lastError = err
		if err == nil && result != nil {
			tc.resultEvent = result
		}
	case cmdAny.MessageIs(&examples.LeaveTable{}):
		result, err := handlers.HandleLeaveTable(eventBook, cmdAny, tc.state)
		tc.lastError = err
		if err == nil && result != nil {
			tc.resultEvent = result
		}
	case cmdAny.MessageIs(&examples.StartHand{}):
		result, err := handlers.HandleStartHand(eventBook, cmdAny, tc.state)
		tc.lastError = err
		if err == nil && result != nil {
			tc.resultEvent = result
		}
	case cmdAny.MessageIs(&examples.EndHand{}):
		result, err := handlers.HandleEndHand(eventBook, cmdAny, tc.state)
		tc.lastError = err
		if err == nil && result != nil {
			tc.resultEvent = result
		}
	default:
		tc.lastError = fmt.Errorf("unknown command type: %s", cmdAny.TypeUrl)
	}

	// Set shared error for common step assertions
	SetLastError(tc.lastError)

	return nil
}

// Then step implementations

func (tc *TableContext) resultIsTableCreated() error {
	if tc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", tc.lastError)
	}
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !tc.resultEvent.MessageIs(&examples.TableCreated{}) {
		return fmt.Errorf("expected TableCreated event, got %s", tc.resultEvent.TypeUrl)
	}
	return nil
}

func (tc *TableContext) resultIsPlayerJoined() error {
	if tc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", tc.lastError)
	}
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !tc.resultEvent.MessageIs(&examples.PlayerJoined{}) {
		return fmt.Errorf("expected PlayerJoined event, got %s", tc.resultEvent.TypeUrl)
	}
	return nil
}

func (tc *TableContext) resultIsPlayerLeft() error {
	if tc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", tc.lastError)
	}
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !tc.resultEvent.MessageIs(&examples.PlayerLeft{}) {
		return fmt.Errorf("expected PlayerLeft event, got %s", tc.resultEvent.TypeUrl)
	}
	return nil
}

func (tc *TableContext) resultIsHandStarted() error {
	if tc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", tc.lastError)
	}
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !tc.resultEvent.MessageIs(&examples.HandStarted{}) {
		return fmt.Errorf("expected HandStarted event, got %s", tc.resultEvent.TypeUrl)
	}
	return nil
}

func (tc *TableContext) resultIsHandEnded() error {
	if tc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", tc.lastError)
	}
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !tc.resultEvent.MessageIs(&examples.HandEnded{}) {
		return fmt.Errorf("expected HandEnded event, got %s", tc.resultEvent.TypeUrl)
	}
	return nil
}

func (tc *TableContext) eventHasTableName(tableName string) error {
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.TableCreated
	if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.TableName != tableName {
		return fmt.Errorf("expected table_name=%s, got %s", tableName, event.TableName)
	}
	return nil
}

func (tc *TableContext) eventHasGameVariant(variant string) error {
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.TableCreated
	if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	expected := examples.GameVariant(examples.GameVariant_value[variant])
	if event.GameVariant != expected {
		return fmt.Errorf("expected game_variant=%s, got %s", variant, event.GameVariant.String())
	}
	return nil
}

func (tc *TableContext) eventHasSmallBlind(amount int) error {
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.TableCreated
	if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.SmallBlind != int64(amount) {
		return fmt.Errorf("expected small_blind=%d, got %d", amount, event.SmallBlind)
	}
	return nil
}

func (tc *TableContext) eventHasBigBlind(amount int) error {
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.TableCreated
	if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.BigBlind != int64(amount) {
		return fmt.Errorf("expected big_blind=%d, got %d", amount, event.BigBlind)
	}
	return nil
}

func (tc *TableContext) eventHasSeatPosition(seat int) error {
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if tc.resultEvent.MessageIs(&examples.PlayerJoined{}) {
		var event examples.PlayerJoined
		if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.SeatPosition != int32(seat) {
			return fmt.Errorf("expected seat_position=%d, got %d", seat, event.SeatPosition)
		}
		return nil
	}
	if tc.resultEvent.MessageIs(&examples.PlayerLeft{}) {
		var event examples.PlayerLeft
		if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.SeatPosition != int32(seat) {
			return fmt.Errorf("expected seat_position=%d, got %d", seat, event.SeatPosition)
		}
		return nil
	}
	return fmt.Errorf("event type %s does not have seat_position", tc.resultEvent.TypeUrl)
}

func (tc *TableContext) eventHasBuyInAmount(amount int) error {
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.PlayerJoined
	if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.BuyInAmount != int64(amount) {
		return fmt.Errorf("expected buy_in_amount=%d, got %d", amount, event.BuyInAmount)
	}
	return nil
}

func (tc *TableContext) eventHasChipsCashedOut(amount int) error {
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.PlayerLeft
	if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.ChipsCashedOut != int64(amount) {
		return fmt.Errorf("expected chips_cashed_out=%d, got %d", amount, event.ChipsCashedOut)
	}
	return nil
}

func (tc *TableContext) eventHasHandNumber(handNumber int) error {
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if tc.resultEvent.MessageIs(&examples.HandStarted{}) {
		var event examples.HandStarted
		if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.HandNumber != int64(handNumber) {
			return fmt.Errorf("expected hand_number=%d, got %d", handNumber, event.HandNumber)
		}
		return nil
	}
	return fmt.Errorf("event type %s does not have hand_number", tc.resultEvent.TypeUrl)
}

func (tc *TableContext) eventHasDealerPosition(position int) error {
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.HandStarted
	if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.DealerPosition != int32(position) {
		return fmt.Errorf("expected dealer_position=%d, got %d", position, event.DealerPosition)
	}
	return nil
}

func (tc *TableContext) eventHasActivePlayers(count int) error {
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.HandStarted
	if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if len(event.ActivePlayers) != count {
		return fmt.Errorf("expected %d active_players, got %d", count, len(event.ActivePlayers))
	}
	return nil
}

func (tc *TableContext) playerStackChangeIs(playerName string, change int) error {
	if tc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.HandEnded
	if err := tc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	playerRoot := tc.getOrCreatePlayerRoot(playerName)
	playerHex := hex.EncodeToString(playerRoot)
	if stackChange, ok := event.StackChanges[playerHex]; ok {
		if stackChange != int64(change) {
			return fmt.Errorf("expected stack change=%d for %s, got %d", change, playerName, stackChange)
		}
		return nil
	}
	return fmt.Errorf("no stack change found for player %s", playerName)
}

func (tc *TableContext) stateHasPlayers(count int) error {
	if tc.state.PlayerCount() != count {
		return fmt.Errorf("expected %d players, got %d", count, tc.state.PlayerCount())
	}
	return nil
}

func (tc *TableContext) stateHasSeatOccupiedBy(seat int, playerName string) error {
	occupant := tc.state.GetSeatOccupant(int32(seat))
	expectedRoot := tc.getOrCreatePlayerRoot(playerName)
	expectedHex := hex.EncodeToString(expectedRoot)
	if occupant != expectedHex {
		return fmt.Errorf("expected seat %d occupied by %s, got %s", seat, playerName, occupant)
	}
	return nil
}

func (tc *TableContext) stateHasStatus(status string) error {
	if tc.state.Status != status {
		return fmt.Errorf("expected status=%s, got %s", status, tc.state.Status)
	}
	return nil
}

func (tc *TableContext) stateHasHandCount(count int) error {
	if tc.state.HandCount != int64(count) {
		return fmt.Errorf("expected hand_count=%d, got %d", count, tc.state.HandCount)
	}
	return nil
}

func (tc *TableContext) commandFailsWith(errorMsg string) error {
	if tc.lastError == nil {
		return fmt.Errorf("expected command to fail, but it succeeded")
	}
	if !strings.Contains(tc.lastError.Error(), errorMsg) {
		return fmt.Errorf("expected error containing '%s', got '%s'", errorMsg, tc.lastError.Error())
	}
	return nil
}
