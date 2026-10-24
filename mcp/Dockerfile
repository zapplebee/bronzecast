FROM python:3.12-slim-bookworm
WORKDIR /app
COPY mcp/requirements.txt /app/mcp/requirements.txt
RUN pip install --no-cache-dir -r /app/mcp/requirements.txt
COPY mcp/ /app/mcp/
ENV PYTHONUNBUFFERED=1
EXPOSE 8090
CMD ["python", "-m", "mcp.server"]
