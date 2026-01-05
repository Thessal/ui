fibonacci(low: Signal<Float>, high: Signal<Float>, level: Float, lookback: Int) : Signal<Float> = {
    g_high : Signal<Float> = ts_max(signal=high, period=lookback)
    g_low  : Signal<Float> = ts_min(signal=low, period=lookback)
    g_range : Signal<Float> = subtract(x=g_high, y=g_low)
    result : Signal<Float> = subtract(x=g_high, y=multiply(x=level, y=g_range))
}