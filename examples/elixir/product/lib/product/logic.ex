defmodule Product.Logic do
  @moduledoc """
  Pure business logic for product aggregate.
  """

  defmodule State do
    defstruct sku: "", name: "", description: "", price_cents: 0, status: :uninitialized

    def exists?(%State{status: status}), do: status != :uninitialized
    def active?(%State{status: status}), do: status == :active
  end

  def rebuild_state(nil), do: %State{}
  def rebuild_state(%{pages: []}), do: %State{}

  def rebuild_state(event_book) do
    Enum.reduce(event_book.pages, %State{}, fn page, state ->
      if page.event, do: apply_event(state, page.event), else: state
    end)
  end

  def handle_create_product(state, sku, name, description, price_cents) do
    cond do
      State.exists?(state) ->
        {:error, :failed_precondition, "Product already exists"}

      is_nil(sku) or sku == "" ->
        {:error, :invalid_argument, "SKU is required"}

      is_nil(name) or name == "" ->
        {:error, :invalid_argument, "Name is required"}

      price_cents <= 0 ->
        {:error, :invalid_argument, "Price must be positive"}

      true ->
        {:ok, Examples.ProductCreated.new(
          sku: sku,
          name: name,
          description: description || "",
          price_cents: price_cents,
          created_at: now_timestamp()
        )}
    end
  end

  def handle_update_product(state, name, description) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Product does not exist"}

      not State.active?(state) ->
        {:error, :failed_precondition, "Product is discontinued"}

      true ->
        {:ok, Examples.ProductUpdated.new(
          name: name || state.name,
          description: description || state.description
        )}
    end
  end

  def handle_set_price(state, price_cents) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Product does not exist"}

      not State.active?(state) ->
        {:error, :failed_precondition, "Product is discontinued"}

      price_cents <= 0 ->
        {:error, :invalid_argument, "Price must be positive"}

      true ->
        {:ok, Examples.PriceSet.new(
          old_price_cents: state.price_cents,
          new_price_cents: price_cents
        )}
    end
  end

  def handle_discontinue(state, reason) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Product does not exist"}

      not State.active?(state) ->
        {:error, :failed_precondition, "Product already discontinued"}

      true ->
        {:ok, Examples.ProductDiscontinued.new(
          reason: reason || "",
          discontinued_at: now_timestamp()
        )}
    end
  end

  defp apply_event(state, event_any) do
    type_url = event_any.type_url

    cond do
      String.ends_with?(type_url, "ProductCreated") ->
        event = Examples.ProductCreated.decode(event_any.value)
        %State{
          sku: event.sku,
          name: event.name,
          description: event.description,
          price_cents: event.price_cents,
          status: :active
        }

      String.ends_with?(type_url, "ProductUpdated") ->
        event = Examples.ProductUpdated.decode(event_any.value)
        %State{state | name: event.name, description: event.description}

      String.ends_with?(type_url, "PriceSet") ->
        event = Examples.PriceSet.decode(event_any.value)
        %State{state | price_cents: event.new_price_cents}

      String.ends_with?(type_url, "ProductDiscontinued") ->
        %State{state | status: :discontinued}

      true ->
        state
    end
  end

  defp now_timestamp do
    Google.Protobuf.Timestamp.new(seconds: System.system_time(:second))
  end
end
