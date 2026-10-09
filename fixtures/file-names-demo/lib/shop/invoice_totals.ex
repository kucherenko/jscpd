defmodule Shop.InvoiceTotals do
  @moduledoc "Sums the lines of an invoice."

  def total(lines, opts \\ []) do
    currency = Keyword.get(opts, :currency, "EUR")

    lines
    |> Enum.reject(&(&1.quantity == 0))
    |> Enum.map(fn line -> line.unit_price * line.quantity end)
    |> Enum.sum()
    |> round_to_cents()
    |> then(&{currency, &1})
  end

  defp round_to_cents(amount) when is_float(amount) do
    Float.round(amount, 2)
  end

  defp round_to_cents(amount), do: amount
end
