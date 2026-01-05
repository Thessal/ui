typ_price_hlc: Signal<Float>(high : Signal<Float>, low : Signal<Float>, close: Signal<Float>) = {

    sum_hl      : Signal<Float> = add(x=high, y=low)
    sum_hlc     : Signal<Float> = add(x=sum_hl, y=close)
    typical_price : Signal<Float> = divide(dividend=sum_hlc, divisor=const(value=3.))

}

true_range: Signal<Float>(high : Signal<Float>, low : Signal<Float>, close: Signal<Float>) = {
    close_prev : Signal<Float> = ts_delay(signal=close, period=1)

    # 3.2. Component 1 = High – Low
    tr1 : Signal<Float> = subtract(x=high, y=low)

    # 3.3. Component 2 = |High – Close(prev)|
    tr2_raw : Signal<Float> = subtract(x=high, y=close_prev)
    tr2     : Signal<Float> = abs(signal=tr2_raw)

    # 3.4. Component 3 = |Low – Close(prev)|
    tr3_raw : Signal<Float> = subtract(x=low, y=close_prev)
    tr3     : Signal<Float> = abs(signal=tr3_raw)

    # 3.5. TR = max(tr1, tr2, tr3)
    tr12 : Signal<Float> = max(x=tr1, y=tr2)
    result: Signal<Float> = max(x=tr12, y=tr3)
}

keltner_band_upper: Signal<Float>(high : Signal<Float>, low : Signal<Float>, close: Signal<Float>, window_size : Int, multiplier  : Float, ema_window  : Int ) = {
    # parameters
    # window_size : Int   = 20            # ATR window
    # multiplier  : Float = 2.            # ATR multiplier
    # ema_window  : Int   = 20            # EMA window for the middle line
    tr : Signal<Float> = true_range(high=high, low=low, close=close)
    atr : Signal<Float> = ts_decay_exp(signal=tr, period=window_size)
    middle_line : Signal<Float> = ts_decay_exp(signal=typical_price, period=ema_window)

    mult_atr   : Signal<Float> = multiply(x=multiplier, y=atr)
    result     : Signal<Float> = add(x=middle_line, y=mult_atr)
}

keltner_band_lower: Signal<Float>(high : Signal<Float>, low : Signal<Float>, close: Signal<Float>, window_size : Int, multiplier  : Float, ema_window  : Int ) = {
    # parameters
    # window_size : Int   = 20            # ATR window
    # multiplier  : Float = 2.            # ATR multiplier
    # ema_window  : Int   = 20            # EMA window for the middle line
    tr : Signal<Float> = true_range(high=high, low=low, close=close)
    atr : Signal<Float> = ts_decay_exp(signal=tr, period=window_size)
    middle_line : Signal<Float> = ts_decay_exp(signal=typical_price, period=ema_window)

    mult_atr   : Signal<Float> = multiply(x=multiplier, y=atr)
    result     : Signal<Float> = subtract(x=middle_line, y=mult_atr)
}