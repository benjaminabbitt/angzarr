package dev.angzarr.examples.player.handlers;

import static org.junit.jupiter.api.Assertions.*;

import dev.angzarr.client.Errors;
import dev.angzarr.examples.Currency;
import dev.angzarr.examples.DepositFunds;
import dev.angzarr.examples.FundsDeposited;
import dev.angzarr.examples.player.state.PlayerState;
import org.junit.jupiter.api.Test;

/** Unit tests for DepositHandler. */
// docs:start:unit_test_deposit
class DepositHandlerTest {

  @Test
  void testDepositIncreasesBankroll() {
    PlayerState state = new PlayerState();
    state.setPlayerId("player_1");
    state.setBankroll(1000);
    DepositFunds cmd =
        DepositFunds.newBuilder()
            .setAmount(Currency.newBuilder().setAmount(500).setCurrencyCode("CHIPS"))
            .build();

    FundsDeposited event = DepositHandler.compute(cmd, state, 500);

    assertEquals(1500, event.getNewBalance().getAmount());
  }

  @Test
  void testDepositRejectsNonExistentPlayer() {
    PlayerState state = new PlayerState(); // playerId empty = doesn't exist

    Exception exception =
        assertThrows(
            Errors.CommandRejectedError.class,
            () -> {
              DepositHandler.guard(state);
            });

    assertTrue(exception.getMessage().contains("does not exist"));
  }

  @Test
  void testDepositRejectsZeroAmount() {
    DepositFunds cmd =
        DepositFunds.newBuilder()
            .setAmount(Currency.newBuilder().setAmount(0).setCurrencyCode("CHIPS"))
            .build();

    Exception exception =
        assertThrows(
            Errors.CommandRejectedError.class,
            () -> {
              DepositHandler.validate(cmd);
            });

    assertTrue(exception.getMessage().contains("positive"));
  }
}
// docs:end:unit_test_deposit
