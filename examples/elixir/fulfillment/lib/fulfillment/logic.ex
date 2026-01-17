defmodule Fulfillment.Logic do
  @moduledoc """
  Pure business logic for fulfillment aggregate.
  State machine: pending -> picking -> packing -> shipped -> delivered
  """

  defmodule State do
    defstruct order_id: "", status: :uninitialized, tracking_number: "",
              carrier: "", shipped_at: nil, delivered_at: nil

    def exists?(%State{status: status}), do: status != :uninitialized
  end

  def rebuild_state(nil), do: %State{}
  def rebuild_state(%{pages: []}), do: %State{}

  def rebuild_state(event_book) do
    Enum.reduce(event_book.pages, %State{}, fn page, state ->
      if page.event, do: apply_event(state, page.event), else: state
    end)
  end

  def handle_create_shipment(state, order_id, items) do
    cond do
      State.exists?(state) ->
        {:error, :failed_precondition, "Shipment already exists"}

      is_nil(order_id) or order_id == "" ->
        {:error, :invalid_argument, "Order ID is required"}

      true ->
        {:ok, Examples.ShipmentCreated.new(
          order_id: order_id,
          items: items || [],
          created_at: now_timestamp()
        )}
    end
  end

  def handle_mark_picked(state) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Shipment does not exist"}

      state.status != :pending ->
        {:error, :failed_precondition, "Cannot pick from status #{state.status}"}

      true ->
        {:ok, Examples.ItemsPicked.new(picked_at: now_timestamp())}
    end
  end

  def handle_mark_packed(state) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Shipment does not exist"}

      state.status != :picking ->
        {:error, :failed_precondition, "Cannot pack from status #{state.status}"}

      true ->
        {:ok, Examples.ItemsPacked.new(packed_at: now_timestamp())}
    end
  end

  def handle_ship(state, tracking_number, carrier) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Shipment does not exist"}

      state.status != :packing ->
        {:error, :failed_precondition, "Cannot ship from status #{state.status}"}

      is_nil(tracking_number) or tracking_number == "" ->
        {:error, :invalid_argument, "Tracking number is required"}

      is_nil(carrier) or carrier == "" ->
        {:error, :invalid_argument, "Carrier is required"}

      true ->
        {:ok, Examples.Shipped.new(
          tracking_number: tracking_number,
          carrier: carrier,
          shipped_at: now_timestamp()
        )}
    end
  end

  def handle_record_delivery(state) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Shipment does not exist"}

      state.status != :shipped ->
        {:error, :failed_precondition, "Cannot deliver from status #{state.status}"}

      true ->
        {:ok, Examples.Delivered.new(delivered_at: now_timestamp())}
    end
  end

  defp apply_event(state, event_any) do
    type_url = event_any.type_url

    cond do
      String.ends_with?(type_url, "ShipmentCreated") ->
        event = Examples.ShipmentCreated.decode(event_any.value)
        %State{state | order_id: event.order_id, status: :pending}

      String.ends_with?(type_url, "ItemsPicked") ->
        %State{state | status: :picking}

      String.ends_with?(type_url, "ItemsPacked") ->
        %State{state | status: :packing}

      String.ends_with?(type_url, "Shipped") ->
        event = Examples.Shipped.decode(event_any.value)
        %State{state |
          status: :shipped,
          tracking_number: event.tracking_number,
          carrier: event.carrier,
          shipped_at: event.shipped_at
        }

      String.ends_with?(type_url, "Delivered") ->
        event = Examples.Delivered.decode(event_any.value)
        %State{state | status: :delivered, delivered_at: event.delivered_at}

      true ->
        state
    end
  end

  defp now_timestamp do
    Google.Protobuf.Timestamp.new(seconds: System.system_time(:second))
  end
end
