#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#include <stdio.h>

int main(void) {
    @autoreleasepool {
        NSArray<id<MTLDevice>> *devices = MTLCopyAllDevices();
        id<MTLDevice> selected = MTLCreateSystemDefaultDevice();
        NSMutableArray *inventory = [NSMutableArray array];
        for (id<MTLDevice> device in devices) {
            [inventory addObject:@{
                @"name": device.name,
                @"registry_id": @(device.registryID),
                @"low_power": @(device.lowPower),
                @"removable": @(device.removable)
            }];
        }
        NSDictionary *record = @{
            @"native_api": @"Metal",
            @"device_count": @(devices.count),
            @"default_device": selected ? selected.name : [NSNull null],
            @"devices": inventory,
            @"command_buffers_submitted": @0
        };
        NSError *error = nil;
        NSData *data = [NSJSONSerialization dataWithJSONObject:record options:NSJSONWritingPrettyPrinted error:&error];
        if (data == nil) { fprintf(stderr, "%s\n", error.localizedDescription.UTF8String); return 3; }
        if (fwrite(data.bytes, 1, data.length, stdout) != data.length || fputc('\n', stdout) == EOF) { return 3; }
        return devices.count == 0 ? 2 : 0;
    }
}
