# \DefaultApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**report_post**](DefaultApi.md#report_post) | **put** /report/{host}/{job}/{time} | 



## report_post

> crate::models::InlineResponse201 report_post(host, job, time, inline_object)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**host** | **String** |  | [required] |
**job** | **String** |  | [required] |
**time** | **i32** |  | [required] |
**inline_object** | [**InlineObject**](InlineObject.md) |  | [required] |

### Return type

[**crate::models::InlineResponse201**](inline_response_201.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

