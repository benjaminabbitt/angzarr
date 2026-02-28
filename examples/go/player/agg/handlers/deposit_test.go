package handlers

import (
	"testing"

	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"github.com/stretchr/testify/assert"
)

// docs:start:unit_test_deposit

func TestDepositIncreasesBankroll(t *testing.T) {
	state := PlayerState{
		PlayerID: "player_1",
		Bankroll: 1000,
	}
	cmd := &examples.DepositFunds{
		Amount: &examples.Currency{Amount: 500, CurrencyCode: "CHIPS"},
	}

	event := computeFundsDeposited(cmd, state, 500)

	assert.Equal(t, int64(1500), event.NewBalance.Amount)
}

func TestDepositRejectsNonExistentPlayer(t *testing.T) {
	state := PlayerState{} // PlayerID empty = doesn't exist

	err := guardDepositFunds(state)

	assert.Error(t, err)
	assert.Contains(t, err.Error(), "does not exist")
}

func TestDepositRejectsZeroAmount(t *testing.T) {
	cmd := &examples.DepositFunds{
		Amount: &examples.Currency{Amount: 0, CurrencyCode: "CHIPS"},
	}

	_, err := validateDepositFunds(cmd)

	assert.Error(t, err)
	assert.Contains(t, err.Error(), "positive")
}

// docs:end:unit_test_deposit
