# Use Python base image.
FROM python:3.12
WORKDIR /app

# Install dependencies.
COPY requirements.txt .
RUN pip install -r requirements.txt

# Copy source code to container.
COPY . .

# Expose port 8000.
EXPOSE 8000

# Default command to run the uvicorn server.
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
