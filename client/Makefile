generate-client:
	docker run --rm \
        -v ${PWD}:/local openapitools/openapi-generator-cli generate \
        -i https://raw.githubusercontent.com/terminusdb/terminusdb/main/docs/openapi.yaml \
        -g rust \
        -o /local/src/openapi_client