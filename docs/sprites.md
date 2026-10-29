# 동물 스프라이트

## 규격
- 프레임 **128×128**, 투명 배경. 시트 한 행 = 한 동작 (행 순서는 `tools/sprite_spec.py` `ANIMS`, Rust `manifest::REQUIRED_ANIMS`와 같다).
- 필수 21동작 뒤에 그 동물만 가진 선택 동작(`OPTIONAL_ANIMS` 순서: 고양이 `arch`·`huff`, 강아지 `wag`)이 이어진다.
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
| hungry | 3 | 시무룩하게 앉아 배 문지르기, 처진 눈썹, 🍙 생각 풍선 | ✓ |
| eat | 4 | 앞에 놓인 밥그릇에 고개 숙여 냠냠 (입 벌림 ↔ 볼 빵빵), 밥알 부스러기 | ✓ |
| level_up | 3 | 만세 (양팔 번쩍, ^^, 활짝 웃음) + 노란 반짝이, 한 번 | ✓ |
| arch (고양이) | 3 | 무서운 척하는 아기 고양이: 둥근 등, 복슬 꼬리, 송곳니 | |
| wag (강아지) | 4 | 혀 내밀고 꼬리 살랑 | ✓ |
| huff (고양이) | 3 | 흥! 반눈·한쪽 볼 빵빵·💢, 고개 돌려 코는 하늘로, 꼬리 탁탁 | ✓ |

## 성장 단계 시트
- 단계: `baby`, `kinder`, `child`, `teenager`, `adult` (`sprite_spec.STAGES`, Rust `care::Stage`). 화면 표시 배율은 `STAGE_SCALE` (0.45 / 0.60 / 0.78 / 1.00 / 1.15).
- 동물마다 `sheet_<stage>.png` 5장을 둔다. 행·열 배치는 모든 단계가 같아서 `pet.json`은 하나다. 단계 시트가 없으면 앱은 `sheet.png`로 대체한다 (기본 그림의 `sheet.png`는 `sheet_teenager.png`와 같은 파일).
- 단계별 비율 (`draw_pets.STAGE_PROPS`): 머리 1.25 / 1.15 / 1.07 / 1.00 / 0.93, 몸 0.80 / 0.88 / 0.94 / 1.00 / 1.06, 다리 0.60 / 0.75 / 0.88 / 1.00 / 1.10. 아기는 머리가 몸보다 확실히 크고 다리가 짧다.
- 외곽선: 작게 표시되는 아기·유아 시트는 외곽선을 굵게 (128 프레임 기준 약 4.6 / 3.8px) 그려 화면에서도 2px 이상 보이게 한다.
- `pet.json`에 `"stages": ["baby", ..., "adult"]`를 적는다.

## pet.json 추가 필드
- `favorite_food`: `{"emoji": "🐟", "name": "생선"}` — 좋아하는 음식 (고양이 🐟 생선, 강아지 🦴 뼈다귀, 토끼 🥕 당근, 곰 🍯 꿀).
- `speech`: `chatter`, `tickle`, `grumpy` (+ 고양이 `hiss`, `ignore`) 그리고 먹이·성장 대사 `hungry`, `full`, `level_up`.
- `personality` (선택): 성격. 고양이만 `"aloof"`.

## 코드 생성기 (기본 그림)
```bash
python tools/draw_pets.py
```
`ui/pets/<id>/{sheet_<stage>.png ×5, sheet.png, pet.json}`을 다시 만들고, 미리보기를 `tools/preview/`에 저장한다(커밋하지 않음):
`all_light.png`·`all_dark.png` (teenager 전체 동작), `stages_<id>.png` (5단계 × walk·roll·sit_front·hungry·eat·level_up (+선택 동작), 오른쪽 열은 화면 표시 크기).
구조: 엔진(슈퍼샘플 마스크·변환 스택) → 종별 설정(색·무늬 데이터) → 포즈(종 공통 뼈대) → 출력. 새 동물은 `Species` 설정 하나를 추가하면 모든 포즈가 그 무늬로 칠해진다. 성장 단계 비율은 공통 조립(`_side`/`_front`)과 머리 부품이 적용하므로 포즈마다 따로 손대지 않는다.

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
