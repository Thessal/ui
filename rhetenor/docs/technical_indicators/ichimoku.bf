# ----------------------------------------------------------------------
#  Function: ichimoku_cloud
#  Returns the “Cloud Top” signal; all other intermediate signals are also
#  defined inside the function scope and can be accessed via their names.
# ----------------------------------------------------------------------
Leading_Span_A: Signal<Float>(high : Signal<Float>, low : Signal<Float>, n1 : Int, n2 : Int) = {
    # typical n1, n2 : 9, 26

    # ----- Tenkan‑sen (Conversion Line) ---------------------------------
    max_n1   : Signal<Float> = ts_max(signal=high, period=n1)
    min_n1   : Signal<Float> = ts_min(signal=low,  period=n1)
    Conversion_Line : Signal<Float> = mid(x=max_n1, y=min_n1)

    # ----- Kijun‑sen (Base Line) ----------------------------------------
    max_n2   : Signal<Float> = ts_max(signal=high, period=n2)
    min_n2   : Signal<Float> = ts_min(signal=low,  period=n2)
    Base_Line : Signal<Float> = mid(x=max_n2, y=min_n2)

    # ----- Senkou Span A (Leading Span A) -------------------------------
    spanA_mid : Signal<Float> = mid(x=Conversion_Line, y=Base_Line)
    result : Signal<Float> = ts_delay(signal=spanA_mid, period=n2)

}

Leading_Span_B: Signal<Float>(high : Signal<Float>, low : Signal<Float>, n3 : Int) = {
    # typical n3 : 52

    # ----- Senkou Span B (Leading Span B) -------------------------------
    max_n3   : Signal<Float> = ts_max(signal=high, period=n3)
    min_n3   : Signal<Float> = ts_min(signal=low,  period=n3)
    spanB_mid : Signal<Float> = mid(x=max_n3, y=min_n3)
    result : Signal<Float> = ts_delay(signal=spanB_mid, period=n2)
}

ichimoku_cloud_top: Signal<Float>(high : Signal<Float>, low : Signal<Float>, n1 : Int, n2 : Int, n3 : Int) = {
    # typical n1, n2, n3 : 9, 26, 52
    result : Signal<Float> = max(x=Leading_Span_A(high, low, n1, n2), y=Leading_Span_B(high, low, n3))
}

ichimoku_cloud_bottom: Signal<Float>(high : Signal<Float>, low : Signal<Float>, n1 : Int, n2 : Int, n3 : Int) = {
    # typical n1, n2, n3 : 9, 26, 52
    result : Signal<Float> = min(x=Leading_Span_A(high, low, n1, n2), y=Leading_Span_B(high, low, n3))
}