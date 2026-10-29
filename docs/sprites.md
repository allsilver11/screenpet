# 동물 스프라이트

## 규격
- 프레임 **128×128**, 투명 배경. 시트 한 행 = 한 동작 (행 순서는 `tools/sprite_spec.py` `ANIMS`, Rust `manifest::REQUIRED_ANIMS`와 같다).
- 필수 18동작 뒤에 그 동물만 가진 선택 동작(`OPTIONAL_ANIMS`: 고양이 `arch`, 강아지 `wag`)이 이어진다.
- 옆모습 동작은 **오른쪽을 보게** 그린다. 왼쪽으로 갈 때는 웹뷰가 좌우 반전한다.
- **앞모습 동작**(`front: true`)은 반전하지 않는다 — 좌우 비대칭 무늬(예: 고양이 왼쪽 뺨 점)가 그대로 보인다.
- 모든 프레임은 가장자리 1px을 비워 둔다(잘림 없음). 서 있는 포즈는 발이 y 120~127에 닿는다.
- **roll 공 규칙**: 웹뷰가 (64, 88)을 축으로 프레임을 돌린다 (`ui/style.css` transform-origin 50% 68.75%). 공은 중심 (64, 88), 반지름 38 원 안에만 그린다. 회전 속도 `spin`은 `sprite_spec.roll_spin(walk_speed)`로 미끄러짐 없이 계산된다.

| 동작 | 권장 장수 | 내용 | 앞모습 |
|---|---|---|---|
| walk | 4 | 걷기 (다리 교차, 몸 살짝 위아래) | |
| run | 4 | 질주 (몸 숙이고 다리 뻗음↔모음, 귀 뒤로) | |
| roll | 2 | 공처럼 만 몸 (원 안), 눈 ^^ | |
| idle | 2 | 옆으로 앉아 숨쉬기 | |
| look | 4 | 앉아서 고개 위/앞/아래/앞 | |
| sleep | 2 | 배 깔고 납작 엎드려 앞발에 턱, 눈 감음, z | |
| hop | 3 | 웅크림 → 쭉 뻗음 → 다리 모음 (한 번) | |
| fall | 2 | 팔다리 벌리고 놀란 눈 | |
| dragged | 2 | 목덜미 잡혀 대롱대롱 | |
| land | 2 | 착지 찌그러짐 → 복귀 (한 번) | |
| sit_front | 4 | 정면 앉기, 마지막 장 깜빡 | ✓ |
| wave | 4 | 앞발 흔들기 (젤리 발바닥) | ✓ |
| tilt | 3 | 고개 갸웃 + `?` | ✓ |
| belly | 4 | 등 대고 누워 배 보이기, 발 꼬물 | ✓ |
| stretch | 3 | 엎드려 기지개 → 하품 → 앉기 (한 번) | ✓ |
| heart | 3 | 하트를 안고 작은 하트가 떠오름 | ✓ |
| tickle | 3 | 몸을 비비 꼬며 깔깔 | ✓ |
| annoyed | 2 | 엎드린 채 실눈·뾰로통·💢 | |
| arch (고양이) | 3 | 등 아치, 꼬리 펑, 하악 | |
| wag (강아지) | 4 | 혀 내밀고 꼬리 살랑 | ✓ |

## 코드 생성기 (기본 그림)
```bash
python tools/draw_pets.py
```
`ui/pets/<id>/{sheet.png, pet.json}`을 다시 만들고, 전체 미리보기를 `tools/preview/all_light.png`, `all_dark.png`에 저장한다(커밋하지 않음).
구조: 엔진(슈퍼샘플 마스크·변환 스택) → 종별 설정(색·무늬 데이터) → 포즈(종 공통 뼈대) → 출력. 새 동물은 `Species` 설정 하나를 추가하면 모든 포즈가 그 무늬로 칠해진다.

## AI 그림으로 바꾸기
동작별 PNG를 만들어 `tools/pack_sheet.py`로 합친다. 파일명: `<동작>_<번호>.png` (0부터).
크기는 자유(예: 512×512)지만 한 동작의 프레임은 같은 캔버스 크기여야 한다. 동작 안에서는 같은 자르기 박스를 써서 움직임이 보존되고, 모든 프레임은 공통 배율로 128×128에 바닥·가운데 정렬된다.

```bash
python tools/pack_sheet.py my_frames/cat ui/pets/cat --name 고양이 --walk-speed 90
```
새 동물은 `ui/pets/<새id>/`로 출력하면 다음 빌드부터 트레이 메뉴에 나타난다.

### 일관성 팁
1. **기준 그림 먼저**: 동물 하나의 `sit_front_0`을 마음에 들 때까지 만든 뒤, 그 이미지를 참조로 넣고 나머지 동작을 만든다.
2. 프롬프트의 스타일 문구를 **모든 프레임에서 그대로 복사**한다.
3. 한 동작의 프레임들은 한 번에 "가로 N칸 스프라이트 시트"로 요청한 뒤 잘라 쓰면 더 일관적이다.
4. 배경 제거가 안 되는 도구면 단색 배경으로 만들고 배경 제거 도구로 투명화한다.

### 프롬프트 템플릿
```
Cute kawaii chibi {동물}, {방향}, {동작 설명}, soft pastel flat colors,
thick rounded brown outline, big head, full body, transparent background,
consistent character, no text
```
예: `{동물}=white and ginger tabby cat`, `{방향}=side view facing right`, `{동작 설명}=walking, left front leg forward (frame 1 of 4)`
