# 🚀 Local n8n Self-Hosting & API Setup Guide

본 문서는 Docker를 사용하여 개인 PC에 n8n을 설치하고, 외부 연동을 위한 API 키를 발급받는 과정을 설명합니다.

## 📋 사전 준비 사항
* **Docker Desktop**: PC에 설치 및 실행 중이어야 합니다. (Engine running 상태 확인)
* **작업 경로**: `C:\Users\Admin\PycharmProjects\main`

---

## 🛠️ 1. n8n 설치 및 실행 (Docker Compose)

### 1.1 설정 파일 작성
프로젝트 루트 폴더(`C:\Users\Admin\PycharmProjects\main`)에 `docker-compose.yml` 파일을 생성하고 아래 내용을 저장합니다.

```yaml
version: '3.8'

services:
  n8n:
    image: n8nio/n8n:latest
    restart: always
    ports:
      - "5678:5678"
    environment:
      - N8N_HOST=localhost
      - N8N_PORT=5678
      - N8N_PROTOCOL=http
      - NODE_ENV=production
      - WEBHOOK_URL=http://localhost:5678/
    volumes:
      - ./n8n_data:/home/node/.n8n # 워크플로우 및 설정 데이터 저장 경로
1.2 컨테이너 실행
터미널(PowerShell)을 열고 아래 명령어를 입력합니다.

PowerShell

# 1. 프로젝트 폴더로 이동
cd C:\Users\Admin\PycharmProjects\main

# 2. n8n 컨테이너 백그라운드 실행
docker-compose up -d
🔑 2. n8n 초기 설정 및 API 키 발급
2.1 관리자 계정 생성
브라우저에서 http://localhost:5678에 접속합니다.

Owner account 설정 화면에서 이메일과 비밀번호를 등록합니다.

초기 설문조사(Customize n8n) 화면은 개인 프로젝트 용도에 맞게 자유롭게 선택 후 완료합니다.

2.2 개인 API 키(API Key) 생성
n8n 메인 화면 좌측 하단의 **Settings (톱니바퀴 아이콘)**를 클릭합니다.

메뉴에서 n8n API 항목을 선택합니다.

Create an API key 버튼을 클릭합니다.

Label에 키의 용도(예: AI_Project_Key)를 입력합니다.

Scopes는 기본값(전체 권한)으로 두고 Save를 누릅니다.

⚠️ 중요: 화면에 표시된 API 키를 즉시 복사하여 안전한 곳에 저장합니다. (창을 닫으면 다시 확인할 수 없습니다.)

🛡️ 보안 및 주의사항
개인용 키: 이 키는 본인 PC에서 돌아가는 n8n 인스턴스 전용 키입니다.

노출 주의: API 키가 외부(GitHub 등)에 노출되지 않도록 주의하세요. .env 파일에 저장하여 사용하는 것을 권장합니다.

데이터 보관: main/n8n_data 폴더에 모든 작업 내용이 저장되므로, 이 폴더를 백업하면 나중에 복구가 가능합니다.