defmodule Cart.Logic do
  @moduledoc """
  Pure business logic for cart aggregate.
  """

  defmodule Item do
    defstruct [:sku, :name, :quantity, :unit_price_cents]
  end

  defmodule State do
    defstruct customer_id: "", items: [], coupon_code: "", discount_cents: 0, status: :uninitialized

    def exists?(%State{status: status}), do: status != :uninitialized
    def active?(%State{status: status}), do: status == :active

    def subtotal_cents(%State{items: items}) do
      Enum.reduce(items, 0, fn item, acc -> acc + item.quantity * item.unit_price_cents end)
    end
  end

  def rebuild_state(nil), do: %State{}
  def rebuild_state(%{pages: []}), do: %State{}

  def rebuild_state(event_book) do
    Enum.reduce(event_book.pages, %State{}, fn page, state ->
      if page.event, do: apply_event(state, page.event), else: state
    end)
  end

  def handle_create_cart(state, customer_id) do
    cond do
      State.exists?(state) ->
        {:error, :failed_precondition, "Cart already exists"}

      is_nil(customer_id) or customer_id == "" ->
        {:error, :invalid_argument, "Customer ID is required"}

      true ->
        {:ok, Examples.CartCreated.new(
          customer_id: customer_id,
          created_at: now_timestamp()
        )}
    end
  end

  def handle_add_item(state, sku, name, quantity, unit_price_cents) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Cart does not exist"}

      not State.active?(state) ->
        {:error, :failed_precondition, "Cart is not active"}

      is_nil(sku) or sku == "" ->
        {:error, :invalid_argument, "SKU is required"}

      quantity <= 0 ->
        {:error, :invalid_argument, "Quantity must be positive"}

      true ->
        {:ok, Examples.ItemAdded.new(
          sku: sku,
          name: name || "",
          quantity: quantity,
          unit_price_cents: unit_price_cents
        )}
    end
  end

  def handle_update_quantity(state, sku, quantity) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Cart does not exist"}

      not State.active?(state) ->
        {:error, :failed_precondition, "Cart is not active"}

      is_nil(sku) or sku == "" ->
        {:error, :invalid_argument, "SKU is required"}

      quantity <= 0 ->
        {:error, :invalid_argument, "Quantity must be positive"}

      true ->
        case Enum.find(state.items, fn i -> i.sku == sku end) do
          nil ->
            {:error, :failed_precondition, "Item #{sku} not in cart"}

          item ->
            {:ok, Examples.QuantityUpdated.new(
              sku: sku,
              old_quantity: item.quantity,
              new_quantity: quantity
            )}
        end
    end
  end

  def handle_remove_item(state, sku) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Cart does not exist"}

      not State.active?(state) ->
        {:error, :failed_precondition, "Cart is not active"}

      is_nil(sku) or sku == "" ->
        {:error, :invalid_argument, "SKU is required"}

      Enum.find(state.items, fn i -> i.sku == sku end) == nil ->
        {:error, :failed_precondition, "Item #{sku} not in cart"}

      true ->
        {:ok, Examples.ItemRemoved.new(sku: sku)}
    end
  end

  def handle_apply_coupon(state, coupon_code, discount_cents) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Cart does not exist"}

      not State.active?(state) ->
        {:error, :failed_precondition, "Cart is not active"}

      is_nil(coupon_code) or coupon_code == "" ->
        {:error, :invalid_argument, "Coupon code is required"}

      state.coupon_code != "" ->
        {:error, :failed_precondition, "Coupon already applied"}

      true ->
        {:ok, Examples.CouponApplied.new(
          coupon_code: coupon_code,
          discount_cents: discount_cents
        )}
    end
  end

  def handle_clear_cart(state) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Cart does not exist"}

      true ->
        {:ok, Examples.CartCleared.new(cleared_at: now_timestamp())}
    end
  end

  def handle_checkout(state, loyalty_points_to_use) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Cart does not exist"}

      not State.active?(state) ->
        {:error, :failed_precondition, "Cart is not active"}

      Enum.empty?(state.items) ->
        {:error, :failed_precondition, "Cart is empty"}

      true ->
        line_items = Enum.map(state.items, fn item ->
          Examples.LineItem.new(
            sku: item.sku,
            name: item.name,
            quantity: item.quantity,
            unit_price_cents: item.unit_price_cents
          )
        end)

        {:ok, Examples.CartCheckoutRequested.new(
          customer_id: state.customer_id,
          items: line_items,
          subtotal_cents: State.subtotal_cents(state),
          discount_cents: state.discount_cents,
          loyalty_points_to_use: loyalty_points_to_use || 0
        )}
    end
  end

  defp apply_event(state, event_any) do
    type_url = event_any.type_url

    cond do
      String.ends_with?(type_url, "CartCreated") ->
        event = Examples.CartCreated.decode(event_any.value)
        %State{state | customer_id: event.customer_id, status: :active}

      String.ends_with?(type_url, "ItemAdded") ->
        event = Examples.ItemAdded.decode(event_any.value)
        new_item = %Item{sku: event.sku, name: event.name, quantity: event.quantity, unit_price_cents: event.unit_price_cents}

        case Enum.find_index(state.items, fn i -> i.sku == event.sku end) do
          nil ->
            %State{state | items: state.items ++ [new_item]}

          idx ->
            existing = Enum.at(state.items, idx)
            updated = %Item{existing | quantity: existing.quantity + event.quantity}
            %State{state | items: List.replace_at(state.items, idx, updated)}
        end

      String.ends_with?(type_url, "QuantityUpdated") ->
        event = Examples.QuantityUpdated.decode(event_any.value)
        items = Enum.map(state.items, fn i ->
          if i.sku == event.sku, do: %Item{i | quantity: event.new_quantity}, else: i
        end)
        %State{state | items: items}

      String.ends_with?(type_url, "ItemRemoved") ->
        event = Examples.ItemRemoved.decode(event_any.value)
        items = Enum.reject(state.items, fn i -> i.sku == event.sku end)
        %State{state | items: items}

      String.ends_with?(type_url, "CouponApplied") ->
        event = Examples.CouponApplied.decode(event_any.value)
        %State{state | coupon_code: event.coupon_code, discount_cents: event.discount_cents}

      String.ends_with?(type_url, "CartCleared") ->
        %State{state | items: [], coupon_code: "", discount_cents: 0, status: :cleared}

      true ->
        state
    end
  end

  defp now_timestamp do
    Google.Protobuf.Timestamp.new(seconds: System.system_time(:second))
  end
end
