"""
Hangul Converter - 영문 키스트로크를 한글로 변환.

pynput으로 캡처된 원시 키스트로크(dkssud)를 한글(안녕)로 변환합니다.
두벌식 키보드 레이아웃 기준.
"""

# 영문 → 한글 자모 매핑 (두벌식)
ENG_TO_KOR = {
    # 자음 (초성/종성 겸용)
    'r': 'ㄱ', 'R': 'ㄲ', 
    's': 'ㄴ', 
    'e': 'ㄷ', 'E': 'ㄸ',
    'f': 'ㄹ', 
    'a': 'ㅁ', 
    'q': 'ㅂ', 'Q': 'ㅃ', 
    't': 'ㅅ', 'T': 'ㅆ', 
    'd': 'ㅇ', 
    'w': 'ㅈ', 'W': 'ㅉ', 
    'c': 'ㅊ',
    'z': 'ㅋ', 
    'x': 'ㅌ', 
    'v': 'ㅍ', 
    'g': 'ㅎ',
    # 모음
    'k': 'ㅏ', 
    'o': 'ㅐ', 
    'i': 'ㅑ', 
    'O': 'ㅒ', 
    'j': 'ㅓ',
    'p': 'ㅔ', 
    'u': 'ㅕ', 
    'P': 'ㅖ', 
    'h': 'ㅗ', 
    'y': 'ㅛ',
    'n': 'ㅜ', 
    'b': 'ㅠ', 
    'm': 'ㅡ', 
    'l': 'ㅣ',
}

# 초성 (19개) - 인덱스 순서 중요
CHOSEONG = ['ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ',
            'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ']

# 중성 (21개)
JUNGSEONG = ['ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ',
             'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ', 'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ']

# 종성 (28개, 첫번째는 없음)
JONGSEONG = ['', 'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ',
             'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ', 'ㅁ', 'ㅂ', 'ㅄ', 'ㅅ',
             'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ']

# 복합 모음
COMPOUND_JUNGSEONG = {
    ('ㅗ', 'ㅏ'): 'ㅘ',
    ('ㅗ', 'ㅐ'): 'ㅙ',
    ('ㅗ', 'ㅣ'): 'ㅚ',
    ('ㅜ', 'ㅓ'): 'ㅝ',
    ('ㅜ', 'ㅔ'): 'ㅞ',
    ('ㅜ', 'ㅣ'): 'ㅟ',
    ('ㅡ', 'ㅣ'): 'ㅢ',
}

# 복합 종성
COMPOUND_JONGSEONG = {
    ('ㄱ', 'ㅅ'): 'ㄳ',
    ('ㄴ', 'ㅈ'): 'ㄵ',
    ('ㄴ', 'ㅎ'): 'ㄶ',
    ('ㄹ', 'ㄱ'): 'ㄺ',
    ('ㄹ', 'ㅁ'): 'ㄻ',
    ('ㄹ', 'ㅂ'): 'ㄼ',
    ('ㄹ', 'ㅅ'): 'ㄽ',
    ('ㄹ', 'ㅌ'): 'ㄾ',
    ('ㄹ', 'ㅍ'): 'ㄿ',
    ('ㄹ', 'ㅎ'): 'ㅀ',
    ('ㅂ', 'ㅅ'): 'ㅄ',
}

# 복합 종성 분리
JONGSEONG_DECOMPOSE = {
    'ㄳ': ('ㄱ', 'ㅅ'),
    'ㄵ': ('ㄴ', 'ㅈ'),
    'ㄶ': ('ㄴ', 'ㅎ'),
    'ㄺ': ('ㄹ', 'ㄱ'),
    'ㄻ': ('ㄹ', 'ㅁ'),
    'ㄼ': ('ㄹ', 'ㅂ'),
    'ㄽ': ('ㄹ', 'ㅅ'),
    'ㄾ': ('ㄹ', 'ㅌ'),
    'ㄿ': ('ㄹ', 'ㅍ'),
    'ㅀ': ('ㄹ', 'ㅎ'),
    'ㅄ': ('ㅂ', 'ㅅ'),
}


def is_choseong(c: str) -> bool:
    return c in CHOSEONG


def is_jungseong(c: str) -> bool:
    return c in JUNGSEONG


def is_jongseong(c: str) -> bool:
    return c in JONGSEONG and c != ''


def compose(cho: str, jung: str, jong: str = '') -> str:
    """초중종으로 한글 음절 조합."""
    try:
        cho_i = CHOSEONG.index(cho)
        jung_i = JUNGSEONG.index(jung)
        jong_i = JONGSEONG.index(jong) if jong else 0
        code = 0xAC00 + (cho_i * 21 * 28) + (jung_i * 28) + jong_i
        return chr(code)
    except ValueError:
        return cho + jung + jong


def convert_to_hangul(text: str) -> str:
    """영문 키스트로크를 한글로 변환."""
    result = []
    
    # 1. 영문 → 자모 변환
    jamos = []
    for c in text:
        if c in ENG_TO_KOR:
            jamos.append(ENG_TO_KOR[c])
        else:
            jamos.append(c)
    
    # 2. 자모 조합
    i = 0
    while i < len(jamos):
        c = jamos[i]
        
        # 자모가 아닌 경우 그대로 출력
        if not (is_choseong(c) or is_jungseong(c)):
            result.append(c)
            i += 1
            continue
        
        # 모음으로 시작하는 경우
        if is_jungseong(c):
            result.append(c)
            i += 1
            continue
        
        # 초성으로 시작
        cho = c
        i += 1
        
        # 다음 글자가 없거나 중성이 아니면 자음만 출력
        if i >= len(jamos) or not is_jungseong(jamos[i]):
            result.append(cho)
            continue
        
        # 중성
        jung = jamos[i]
        i += 1
        
        # 복합 모음 체크
        if i < len(jamos) and is_jungseong(jamos[i]):
            compound = COMPOUND_JUNGSEONG.get((jung, jamos[i]))
            if compound:
                jung = compound
                i += 1
        
        # 종성 체크
        jong = ''
        if i < len(jamos) and is_choseong(jamos[i]) and jamos[i] in JONGSEONG:
            potential_jong = jamos[i]
            
            # 다음이 모음이면 종성이 아니라 다음 글자 초성
            if i + 1 < len(jamos) and is_jungseong(jamos[i + 1]):
                # 종성 없이 완성
                result.append(compose(cho, jung, ''))
                continue
            
            # 종성으로 사용
            jong = potential_jong
            i += 1
            
            # 복합 종성 체크
            if i < len(jamos) and is_choseong(jamos[i]):
                next_c = jamos[i]
                compound_jong = COMPOUND_JONGSEONG.get((jong, next_c))
                
                if compound_jong:
                    # 다다음이 모음이면 복합 종성 불가, 앞 종성만 사용
                    if i + 1 < len(jamos) and is_jungseong(jamos[i + 1]):
                        # jong 유지, 다음 글자 처리
                        pass
                    else:
                        jong = compound_jong
                        i += 1
            
            # 종성 다음이 모음이면 종성을 쪼개거나 넘겨야 함
            if i < len(jamos) and is_jungseong(jamos[i]):
                if jong in JONGSEONG_DECOMPOSE:
                    # 복합 종성 분리
                    first, second = JONGSEONG_DECOMPOSE[jong]
                    result.append(compose(cho, jung, first))
                    # second를 다음 초성으로
                    cho = second
                    jung = jamos[i]
                    i += 1
                    jong = ''
                    
                    # 다시 복합 모음 체크
                    if i < len(jamos) and is_jungseong(jamos[i]):
                        compound = COMPOUND_JUNGSEONG.get((jung, jamos[i]))
                        if compound:
                            jung = compound
                            i += 1
                    
                    # 다시 종성 체크
                    if i < len(jamos) and is_choseong(jamos[i]) and jamos[i] in JONGSEONG:
                        if i + 1 >= len(jamos) or not is_jungseong(jamos[i + 1]):
                            jong = jamos[i]
                            i += 1
                    
                    result.append(compose(cho, jung, jong))
                    continue
                else:
                    # 단일 종성을 다음 초성으로 넘김
                    result.append(compose(cho, jung, ''))
                    i -= 1  # 종성을 다시 초성으로 처리
                    continue
        
        result.append(compose(cho, jung, jong))
    
    return ''.join(result)


if __name__ == "__main__":
    test_cases = [
        ("dkssud", "안녕"),
        ("dkssudgktpdy", "안녕하세요"),
        ("rkatkgkqslek", "감사합니다"),
        ("gksrmf", "한글"),
        ("hello", "hello"),
        ("dkssud hello", "안녕 hello"),
        ("tlfgod", "시공"),
        ("Rhkqo", "꽈배"),
    ]
    
    print("=== 한글 변환 테스트 ===")
    for eng, expected in test_cases:
        result = convert_to_hangul(eng)
        status = "✓" if result == expected else "✗"
        print(f"{status} '{eng}' → '{result}' (expected: '{expected}')")
