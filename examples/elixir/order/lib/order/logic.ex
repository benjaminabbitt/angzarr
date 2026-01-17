defmodule Order.Logic do
  @moduledoc """
  Pure business logic for order aggregate.
  """

  defmodule State do
    defstruct customer_id: "", items: [], subtotal_cents: 0, discount_cents: 0,
              loyalty_points_used: 0, final_total_cents: 0, payment_method: "",
              status: :uninitialized

    def exists?(%State{status: status}), do: status != :uninitialized
    def pending_payment?(%State{status: status}), do: status == :pending_payment
    def payment_submitted?(%State{status: status}), do: status == :payment_submitted
  end

  def rebuild_state(nil), do: %State{}
  def rebuild_state(%{pages: []}), do: %State{}

  def rebuild_state(event_book) do
    Enum.reduce(event_book.pages, %State{}, fn page, state ->
      if page.event, do: apply_event(state, page.event), else: state
    end)
  end

  def handle_create_order(state, customer_id, items, subtotal_cents, discount_cents) do
    cond do
      State.exists?(state) ->
        {:error, :failed_precondition, "Order already exists"}

      is_nil(customer_id) or customer_id == "" ->
        {:error, :invalid_argument, "Customer ID is required"}

      is_nil(items) or Enum.empty?(items) ->
        {:error, :invalid_argument, "Order must have items"}

      true ->
        {:ok, Examples.OrderCreated.new(
          customer_id: customer_id,
          items: items,
          subtotal_cents: subtotal_cents,
          discount_cents: discount_cents,
          created_at: now_timestamp()
        )}
    end
  end

  def handle_apply_loyalty_discount(state, points_used, discount_cents) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Order does not exist"}

      not State.pending_payment?(state) ->
        {:error, :failed_precondition, "Order is not pending payment"}

      points_used <= 0 ->
        {:error, :invalid_argument, "Points must be positive"}

      discount_cents <= 0 ->
        {:error, :invalid_argument, "Discount must be positive"}

      true ->
        {:ok, Examples.LoyaltyDiscountApplied.new(
          points_used: points_used,
          discount_cents: discount_cents
        )}
    end
  end

  def handle_submit_payment(state, payment_method, amount_cents) do
    expected_total = state.subtotal_cents - state.discount_cents

    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Order does not exist"}

      not State.pending_payment?(state) ->
        {:error, :failed_precondition, "Order is not pending payment"}

      is_nil(payment_method) or payment_method == "" ->
        {:error, :invalid_argument, "Payment method is required"}

      amount_cents <= 0 ->
        {:error, :invalid_argument, "Amount must be positive"}

      amount_cents != expected_total ->
        {:error, :invalid_argument, "Payment amount #{amount_cents} does not match expected #{expected_total}"}

      true ->
        {:ok, Examples.PaymentSubmitted.new(
          payment_method: payment_method,
          amount_cents: amount_cents,
          submitted_at: now_timestamp()
        )}
    end
  end

  def handle_confirm_payment(state) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Order does not exist"}

      not State.payment_submitted?(state) ->
        {:error, :failed_precondition, "Payment not submitted"}

      true ->
        {:ok, Examples.PaymentConfirmed.new(confirmed_at: now_timestamp())}
    end
  end

  def handle_complete_order(state) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Order does not exist"}

      true ->
        {:ok, Examples.OrderCompleted.new(completed_at: now_timestamp())}
    end
  end

  def handle_cancel_order(state, reason, loyalty_points_used) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Order does not exist"}

      state.status == :completed ->
        {:error, :failed_precondition, "Cannot cancel completed order"}

      true ->
        {:ok, Examples.OrderCancelled.new(
          reason: reason || "",
          loyalty_points_used: loyalty_points_used || state.loyalty_points_used,
          cancelled_at: now_timestamp()
        )}
    end
  end

  defp apply_event(state, event_any) do
    type_url = event_any.type_url

    cond do
      String.ends_with?(type_url, "OrderCreated") ->
        event = Examples.OrderCreated.decode(event_any.value)
        %State{state |
          customer_id: event.customer_id,
          items: event.items,
          subtotal_cents: event.subtotal_cents,
          discount_cents: event.discount_cents,
          status: :pending_payment
        }

      String.ends_with?(type_url, "LoyaltyDiscountApplied") ->
        event = Examples.LoyaltyDiscountApplied.decode(event_any.value)
        %State{state |
          loyalty_points_used: event.points_used,
          discount_cents: state.discount_cents + event.discount_cents
        }

      String.ends_with?(type_url, "PaymentSubmitted") ->
        event = Examples.PaymentSubmitted.decode(event_any.value)
        %State{state |
          payment_method: event.payment_method,
          final_total_cents: event.amount_cents,
          status: :payment_submitted
        }

      String.ends_with?(type_url, "PaymentConfirmed") ->
        %State{state | status: :payment_confirmed}

      String.ends_with?(type_url, "OrderCompleted") ->
        %State{state | status: :completed}

      String.ends_with?(type_url, "OrderCancelled") ->
        %State{state | status: :cancelled}

      true ->
        state
    end
  end

  defp now_timestamp do
    Google.Protobuf.Timestamp.new(seconds: System.system_time(:second))
  end
end
