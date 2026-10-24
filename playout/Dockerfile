FROM python:3.12-slim-bookworm
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY playout/requirements.txt /app/playout/requirements.txt
RUN pip install --no-cache-dir -r /app/playout/requirements.txt
COPY playout/ /app/playout/
COPY web/ /app/web/
ENV PYTHONUNBUFFERED=1
EXPOSE 8080 5000/udp
CMD ["python", "-m", "playout.server"]
