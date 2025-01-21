#!/bin/bash

# Set variables
NAMESPACE="gpu-share"
REGISTRY="dantegpu.com"
IMAGE_NAME="gpu-share-manager"
TAG="latest"

# Build Docker image
echo "Building Docker image..."
docker build -t ${REGISTRY}/${IMAGE_NAME}:${TAG} .

# Push Docker image
echo "Pushing Docker image to registry..."
docker push ${REGISTRY}/${IMAGE_NAME}:${TAG}

# Create namespace if it doesn't exist
kubectl create namespace ${NAMESPACE} --dry-run=client -o yaml | kubectl apply -f -

# Apply Kubernetes configurations
echo "Applying Kubernetes configurations..."
kubectl apply -f kubernetes/config.yaml
kubectl apply -f kubernetes/storage.yaml
kubectl apply -f kubernetes/deployment.yaml
kubectl apply -f kubernetes/service.yaml

# Wait for deployment to be ready
echo "Waiting for deployment to be ready..."
kubectl rollout status deployment/gpu-share-manager -n ${NAMESPACE}

echo "Deployment completed successfully!"