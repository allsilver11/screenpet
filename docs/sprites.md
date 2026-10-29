# 동물 스프라이트 만들기 (AI 생성)

## 규격
- 동작별로 PNG를 따로 만들고 `tools/pack_sheet.py`로 합친다. 파일명: `<동작>_<번호>.png` (0부터).
- 투명 배경, **오른쪽을 보는** 옆모습, 발이 그림 맨 아래에 닿게.
- 크기는 자유(예: 512×512). 도구가 공통 배율로 64×64에 맞춰 바닥·가운데 정렬한다.

| 동작 | 권장 장수 | 내용 |
|---|---|---|
| walk | 4 | 걷기 한 사이클 (다리 교차) |
| idle | 2 | 앉아서 숨쉬기 (살짝 위아래) |
| look | 4 | 두리번거리기 (정면→옆→정면→반대) |
| sleep | 2 | 웅크리고 자기 (호흡) |
| hop | 3 | 도약 준비 → 공중 → 하강 |
| fall | 2 | 놀라서 팔다리 벌리고 떨어짐 |
| dragged | 2 | 목덜미 잡혀 대롱대롱 |
| land | 2 | 착지 찌그러짐 → 복귀 |

## 일관성 팁
1. **기준 그림 먼저**: 동물 하나의 `idle_0`를 마음에 들 때까지 만든 뒤, 그 이미지를 참조 이미지로 넣고 나머지 동작을 만든다.
2. 프롬프트의 스타일 문구를 **모든 프레임에서 그대로 복사**한다.
3. 한 동작의 프레임들은 한 번에 "가로 N칸 스프라이트 시트"로 요청한 뒤 잘라 쓰면 더 일관적이다.
4. 배경 제거가 안 되는 도구면 단색 배경으로 만들고 배경 제거 도구로 투명화한다.

## 프롬프트 템플릿
```
Cute chibi {동물} side view facing right, {동작 설명}, pixel art style,
64x64 sprite, thick dark outline, flat pastel colors, full body, feet touching
the bottom edge, transparent background, consistent character, no text
```
예: `{동물}=orange tabby cat`, `{동작 설명}=walking, left front leg forward (frame 1 of 4)`

## 합치기
```bash
python tools/pack_sheet.py my_frames/cat ui/pets/cat --name 고양이 --walk-speed 60
```
새 동물은 `ui/pets/<새id>/`로 출력하면 다음 빌드부터 트레이 메뉴에 나타난다.
