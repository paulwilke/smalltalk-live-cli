"
Default error handler of a headless image while the stlive server runs. Pharo's own default for processes
without a handler (the NonInteractiveUIManager) logs the error and QUITS the image, so one unhandled error
in a forked process used to end the whole session. This handler logs the error (instance log, `image info`),
terminates only the failing process and keeps the image running. Warnings and notifications keep the
behaviour of the previous handler; stlive's own jobs never reach this handler (they have their own).
"
Class {
	#name : 'StLiveBackgroundErrorHandler',
	#superclass : 'Object',
	#instVars : [
		'server',
		'fallback'
	],
	#package : 'StLive'
}

{ #category : 'accessing' }
StLiveBackgroundErrorHandler >> server: aServer fallback: anErrorHandler [
	server := aServer.
	fallback := anErrorHandler
]

{ #category : 'accessing' }
StLiveBackgroundErrorHandler >> fallback [
	^ fallback
]

{ #category : 'handling' }
StLiveBackgroundErrorHandler >> handleError: anError [
	"Runs on top of the failing stack in the failing process: record, log, end only that process."
	[ server noteBackgroundError: anError in: Processor activeProcess ] on: Error do: [ :e | nil ].
	Processor activeProcess terminate
]

{ #category : 'handling' }
StLiveBackgroundErrorHandler >> handleWarning: aWarning [
	^ fallback handleWarning: aWarning
]

{ #category : 'handling' }
StLiveBackgroundErrorHandler >> handleNotification: aNotification [
	^ fallback handleNotification: aNotification
]

{ #category : 'handling' }
StLiveBackgroundErrorHandler >> doesNotUnderstand: aMessage [
	^ aMessage sendTo: fallback
]
