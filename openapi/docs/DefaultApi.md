# \DefaultApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**enrol**](DefaultApi.md#enrol) | **post** /enrol | 
[**global_jobs**](DefaultApi.md#global_jobs) | **get** /global/jobs | 
[**report_finish**](DefaultApi.md#report_finish) | **post** /report/finish | 
[**report_output**](DefaultApi.md#report_output) | **post** /report/output | 
[**report_start**](DefaultApi.md#report_start) | **post** /report/start | 



## enrol

> enrol(enrol_body)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**enrol_body** | [**EnrolBody**](EnrolBody.md) |  | [required] |

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## global_jobs

> crate::models::GlobalJobsResult global_jobs()


### Parameters

This endpoint does not need any parameter.

### Return type

[**crate::models::GlobalJobsResult**](GlobalJobsResult.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## report_finish

> crate::models::ReportResult report_finish(report_finish_body)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**report_finish_body** | [**ReportFinishBody**](ReportFinishBody.md) |  | [required] |

### Return type

[**crate::models::ReportResult**](ReportResult.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## report_output

> crate::models::ReportResult report_output(report_output_body)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**report_output_body** | [**ReportOutputBody**](ReportOutputBody.md) |  | [required] |

### Return type

[**crate::models::ReportResult**](ReportResult.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## report_start

> crate::models::ReportResult report_start(report_start_body)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**report_start_body** | [**ReportStartBody**](ReportStartBody.md) |  | [required] |

### Return type

[**crate::models::ReportResult**](ReportResult.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

