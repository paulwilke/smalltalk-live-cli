"
A well-defined, reportable command failure (bad arguments, unknown reference, ...).
Turned into an { ok: false, error: { code, message } } response.
"
Class {
	#name : 'StLiveFailure',
	#superclass : 'Error',
	#instVars : [
		'code',
		'details'
	],
	#package : 'StLive'
}

{ #category : 'instance creation' }
StLiveFailure class >> code: aString message: aMessage [
	^ self new code: aString; details: Dictionary new; messageText: aMessage; yourself
]

{ #category : 'instance creation' }
StLiveFailure class >> code: aString message: aMessage details: aDictionary [
	^ self new code: aString; details: aDictionary; messageText: aMessage; yourself
]

{ #category : 'accessing' }
StLiveFailure >> code [
	^ code
]

{ #category : 'accessing' }
StLiveFailure >> code: aString [
	code := aString
]

{ #category : 'accessing' }
StLiveFailure >> details [
	^ details
]

{ #category : 'accessing' }
StLiveFailure >> details: aDictionary [
	details := aDictionary
]

{ #category : 'converting' }
StLiveFailure >> asError [
	| d |
	d := OrderedDictionary new.
	d at: 'code' put: code; at: 'message' put: self messageText.
	details keysAndValuesDo: [ :k :v | d at: k put: v ].
	^ d
]
