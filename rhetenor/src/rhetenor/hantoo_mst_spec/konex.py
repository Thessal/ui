
# KONEX Master File Spec
URL = "https://new.real.download.dws.co.kr/common/master/konex_code.mst.zip"
FILENAME = "konex_code.mst"
PARSER_TYPE = "fixed_suffix"
SUFFIX_LENGTH = 184

# Derived from kis_konex_code_mst.py slices:
# scrt_grp_cls_code = row[-184:-182].strip() -> 2
# stck_sdpr = row[-182:-173].strip() -> 9
# frml_mrkt_deal_qty_unit = row[-173:-168].strip() -> 5
# ovtm_mrkt_deal_qty_unit = row[-168:-163].strip() -> 5
# trht_yn = row[-163:-162].strip() -> 1
# sltr_yn = row[-162:-161].strip() -> 1
# mang_issu_yn = row[-161:-160].strip() -> 1
# mrkt_alrm_cls_code = row[-160:-158].strip() -> 2
# mrkt_alrm_risk_adnt_yn = row[-158:-157].strip() -> 1
# insn_pbnt_yn = row[-157:-156].strip() -> 1
# byps_lstn_yn = row[-156:-155].strip() -> 1
# flng_cls_code = row[-155:-153].strip() -> 2
# fcam_mod_cls_code = row[-153:-151].strip() -> 2
# icic_cls_code = row[-151:-149].strip() -> 2
# marg_rate = row[-149:-146].strip() -> 3
# crdt_able = row[-146:-145].strip() -> 1
# crdt_days = row[-145:-142].strip() -> 3
# prdy_vol = row[-142:-130].strip() -> 12
# stck_fcam = row[-130:-118].strip() -> 12
# stck_lstn_date = row[-118:-110].strip() -> 8
# lstn_stcn = row[-110:-95].strip() -> 15
# cpfn = row[-95:-74].strip() -> 21
# stac_month = row[-74:-72].strip() -> 2
# po_prc = row[-72:-65].strip() -> 7
# prst_cls_code = row[-65:-64].strip() -> 1
# ssts_hot_yn = row[-64:-63].strip() -> 1
# stange_runup_yn = row[-63:-62].strip() -> 1
# krx300_issu_yn = row[-62:-61].strip() -> 1
# sale_account = row[-61:-52].strip() -> 9
# bsop_prfi = row[-52:-43].strip() -> 9
# op_prfi = row[-43:-34].strip() -> 9
# thtr_ntin = row[-34:-29].strip() -> 5
# roe = row[-29:-20].strip() -> 9
# base_date = row[-20:-12].strip() -> 8
# prdy_avls_scal = row[-12:-3].strip() -> 9
# co_crdt_limt_over_yn = row[-3:-2].strip() -> 1
# secu_lend_able_yn = row[-2:-1].strip() -> 1
# stln_able_yn = row[-1:].strip() -> 1

FIELD_SPECS = [
    2, 9, 5, 5, 1, 
    1, 1, 2, 1, 1, 
    1, 2, 2, 2, 3, 
    1, 3, 12, 12, 8, 
    15, 21, 2, 7, 1, 
    1, 1, 1, 9, 9, 
    9, 5, 9, 8, 9, 
    1, 1, 1
]

COLUMNS = [
    '증권그룹구분코드', '주식 기준가', '정규 시장 매매 수량 단위', '시간외 시장 매매 수량 단위', '거래정지 여부', 
    '정리매매 여부', '관리 종목 여부', '시장 경고 구분 코드', '시장 경고위험 예고 여부', '불성실 공시 여부', 
    '우회 상장 여부', '락구분 코드', '액면가 변경 구분 코드', '증자 구분 코드', '증거금 비율', 
    '신용주문 가능 여부', '신용기간', '전일 거래량', '주식 액면가', '주식 상장 일자', 
    '상장 주수(천)', '자본금', '결산 월', '공모 가격', '우선주 구분 코드', 
    '공매도과열종목여부', '이상급등종목여부', 'KRX300 종목 여부', '매출액', '영업이익', 
    '경상이익', '단기순이익', 'ROE', '기준년월', '전일기준 시가총액(억)', 
    '회사신용한도초과여부', '담보대출가능여부', '대주가능여부'
]
