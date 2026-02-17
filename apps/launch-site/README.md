# NOVA Launch Site (Phase 1)

`main` 저장소 내 마케팅/런칭 사이트 앱입니다.  
Astro + Tailwind 기반 정적 사이트이며, GitHub Pages 배포를 기준으로 구성했습니다.

## Tech Stack

- Astro (static output)
- Tailwind CSS
- Astro file-based routing
- GitHub Actions + GitHub Pages

## Run Locally

```bash
cd apps/launch-site
npm ci
npm run dev
```

개발 서버: `http://localhost:4321`

## Build

```bash
cd apps/launch-site
npm run build
npm run preview
```

빌드 산출물: `dist/`

## Routing

- `/` Home
- `/product` Product
- `/solutions` Solutions (B2B)
- `/pricing` Pricing
- `/docs` Docs Landing
- `/docs/getting-started`
- `/docs/tutorials`
- `/docs/api`
- `/docs/faq`
- `/docs/changelog`
- `/updates` Updates
- `/company` Company (+ Contact 섹션 포함)

## Project Structure

```text
src/
  components/
    ui/
  layouts/
  pages/
    docs/
    updates/
  styles/
public/
.github/workflows/deploy-launch-site.yml
astro.config.mjs
```

## GitHub Pages Deploy

워크플로우 파일: `.github/workflows/deploy-launch-site.yml`

- `main` 브랜치에 `apps/launch-site/**` 변경 푸시 시 자동 배포
- GitHub Pages artifact 경로: `apps/launch-site/dist`
- `astro.config.mjs` 설정:
  - `output: 'static'`
  - `site`: `https://{owner}.github.io` (기본값)
  - `base`: Actions에서는 저장소명 기준 자동 계산 (`/main/`), 로컬에서는 `/`

필수 저장소 설정:

1. GitHub Repository > `Settings` > `Pages`
2. Source를 `GitHub Actions`로 선택
3. `main` 브랜치에 푸시하면 자동 배포

## Notes

- 콘텐츠는 1차 빌드 기준 placeholder 중심으로 구성됨
- SEO 기본 메타/OG, `robots.txt`, `sitemap` 생성 포함
- `prefers-reduced-motion` 지원 및 키보드 포커스 스타일 적용
