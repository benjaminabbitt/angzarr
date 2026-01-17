defmodule Inventory.Logic do
  @moduledoc """
  Pure business logic for inventory aggregate.
  """

  @low_stock_threshold 10

  defmodule Reservation do
    defstruct [:order_id, :quantity]
  end

  defmodule State do
    defstruct sku: "", on_hand: 0, reserved: 0, reservations: [], low_stock_threshold: 10

    def exists?(%State{sku: sku}), do: sku != ""
    def available(%State{on_hand: on_hand, reserved: reserved}), do: on_hand - reserved
  end

  def rebuild_state(nil), do: %State{}
  def rebuild_state(%{pages: []}), do: %State{}

  def rebuild_state(event_book) do
    Enum.reduce(event_book.pages, %State{}, fn page, state ->
      if page.event, do: apply_event(state, page.event), else: state
    end)
  end

  def handle_initialize_stock(state, sku, initial_quantity, low_stock_threshold) do
    cond do
      State.exists?(state) ->
        {:error, :failed_precondition, "Inventory already exists"}

      is_nil(sku) or sku == "" ->
        {:error, :invalid_argument, "SKU is required"}

      initial_quantity < 0 ->
        {:error, :invalid_argument, "Quantity must be non-negative"}

      true ->
        {:ok, Examples.StockInitialized.new(
          sku: sku,
          initial_quantity: initial_quantity,
          low_stock_threshold: low_stock_threshold || @low_stock_threshold
        )}
    end
  end

  def handle_receive_stock(state, quantity, reference) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Inventory does not exist"}

      quantity <= 0 ->
        {:error, :invalid_argument, "Quantity must be positive"}

      true ->
        {:ok, Examples.StockReceived.new(
          quantity: quantity,
          new_on_hand: state.on_hand + quantity,
          reference: reference || ""
        )}
    end
  end

  def handle_reserve_stock(state, order_id, quantity) do
    available = State.available(state)

    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Inventory does not exist"}

      is_nil(order_id) or order_id == "" ->
        {:error, :invalid_argument, "Order ID is required"}

      quantity <= 0 ->
        {:error, :invalid_argument, "Quantity must be positive"}

      quantity > available ->
        {:error, :failed_precondition, "Insufficient stock: available #{available}, requested #{quantity}"}

      Enum.find(state.reservations, fn r -> r.order_id == order_id end) != nil ->
        {:error, :failed_precondition, "Reservation already exists for order #{order_id}"}

      true ->
        new_available = available - quantity
        events = [
          Examples.StockReserved.new(
            order_id: order_id,
            quantity: quantity,
            new_available: new_available
          )
        ]

        events = if new_available <= state.low_stock_threshold and available > state.low_stock_threshold do
          events ++ [Examples.LowStockAlert.new(
            sku: state.sku,
            current_available: new_available,
            threshold: state.low_stock_threshold
          )]
        else
          events
        end

        {:ok, events}
    end
  end

  def handle_release_reservation(state, order_id) do
    reservation = Enum.find(state.reservations, fn r -> r.order_id == order_id end)

    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Inventory does not exist"}

      is_nil(order_id) or order_id == "" ->
        {:error, :invalid_argument, "Order ID is required"}

      reservation == nil ->
        {:error, :failed_precondition, "No reservation found for order #{order_id}"}

      true ->
        {:ok, Examples.ReservationReleased.new(
          order_id: order_id,
          quantity: reservation.quantity,
          new_available: State.available(state) + reservation.quantity
        )}
    end
  end

  def handle_commit_reservation(state, order_id) do
    reservation = Enum.find(state.reservations, fn r -> r.order_id == order_id end)

    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Inventory does not exist"}

      is_nil(order_id) or order_id == "" ->
        {:error, :invalid_argument, "Order ID is required"}

      reservation == nil ->
        {:error, :failed_precondition, "No reservation found for order #{order_id}"}

      true ->
        {:ok, Examples.ReservationCommitted.new(
          order_id: order_id,
          quantity: reservation.quantity,
          new_on_hand: state.on_hand - reservation.quantity
        )}
    end
  end

  defp apply_event(state, event_any) do
    type_url = event_any.type_url

    cond do
      String.ends_with?(type_url, "StockInitialized") ->
        event = Examples.StockInitialized.decode(event_any.value)
        %State{
          sku: event.sku,
          on_hand: event.initial_quantity,
          reserved: 0,
          reservations: [],
          low_stock_threshold: event.low_stock_threshold
        }

      String.ends_with?(type_url, "StockReceived") ->
        event = Examples.StockReceived.decode(event_any.value)
        %State{state | on_hand: event.new_on_hand}

      String.ends_with?(type_url, "StockReserved") ->
        event = Examples.StockReserved.decode(event_any.value)
        new_reservation = %Reservation{order_id: event.order_id, quantity: event.quantity}
        %State{state |
          reserved: state.reserved + event.quantity,
          reservations: state.reservations ++ [new_reservation]
        }

      String.ends_with?(type_url, "ReservationReleased") ->
        event = Examples.ReservationReleased.decode(event_any.value)
        reservations = Enum.reject(state.reservations, fn r -> r.order_id == event.order_id end)
        %State{state |
          reserved: state.reserved - event.quantity,
          reservations: reservations
        }

      String.ends_with?(type_url, "ReservationCommitted") ->
        event = Examples.ReservationCommitted.decode(event_any.value)
        reservations = Enum.reject(state.reservations, fn r -> r.order_id == event.order_id end)
        %State{state |
          on_hand: event.new_on_hand,
          reserved: state.reserved - event.quantity,
          reservations: reservations
        }

      true ->
        state
    end
  end
end
