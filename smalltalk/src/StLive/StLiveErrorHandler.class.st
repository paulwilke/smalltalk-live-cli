"
Installed as the errorHandler of a job's worker process. Runs *inside* the failing
execution (on top of the signal context), registers a debug session and parks the process.
Resuming the process makes #handleError: return, which resumes the signalling code.
"
Class {
	#name : 'StLiveErrorHandler',
	#superclass : 'Object',
	#instVars : [
		'job'
	],
	#package : 'StLive'
}

{ #category : 'accessing' }
StLiveErrorHandler >> job: aJob [
	job := aJob
]

{ #category : 'accessing' }
StLiveErrorHandler >> job [
	^ job
]

{ #category : 'handling' }
StLiveErrorHandler >> handleError: anError [
	| session |
	session := job server
		newSessionFor: job
		exception: anError
		context: anError signalerContext
		kind: #exception.
	job erroredWith: session.
	Processor activeProcess suspend.
	^ session takeResumeValue
]

{ #category : 'handling' }
StLiveErrorHandler >> handleNotification: aNotification [
	"Unhandled notifications and warnings are not errors: let them run their default action."
	^ ErrorHandler defaultErrorHandler handleNotification: aNotification
]

{ #category : 'handling' }
StLiveErrorHandler >> doesNotUnderstand: aMessage [
	"Everything but errors (warnings, notifications, ...) keeps its default behaviour."
	^ aMessage sendTo: ErrorHandler defaultErrorHandler
]
