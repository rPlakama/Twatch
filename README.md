TODO:

- Making it be able to run in any path, so its not /src dependent. 
- Making the prints cleaner 
- Creating a selection list for sessions

HOW IT WORKS:

The project current works by capturing information from /sys/class/hwmon/ and classifying devices by it labels or information inside those files. The project current lacks more elegance, and maybe, removing python as a dependencies for plot creation. At the current moment, the project have those functions:

- Plot latest
    
    _Servers as a complement for the Raw session, and maybe just seeing the result again._
- Trigger 
   
   _This where it sets two different ways of auto-finishing and plotting based in the number of captures and the target temperatures._

- What is this project useful for?
  
  _Mere having graphics for 'what took to reach x temperature' or 'how my hardware did during a defined time'_

