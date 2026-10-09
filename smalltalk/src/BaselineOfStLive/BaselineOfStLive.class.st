Class {
	#name : 'BaselineOfStLive',
	#superclass : 'BaselineOf',
	#package : 'BaselineOfStLive'
}

{ #category : 'baselines' }
BaselineOfStLive >> baseline: spec [
	<baseline>
	spec for: #common do: [ spec package: 'StLive' ]
]
